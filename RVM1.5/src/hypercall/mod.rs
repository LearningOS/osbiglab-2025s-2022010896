use core::convert::TryFrom;
use core::sync::atomic::{AtomicU32, Ordering};

use bit_field::BitField;
use numeric_enum_macro::numeric_enum;

use crate::arch::{vmm::VcpuAccessGuestState, GuestPageTableImmut};
use crate::error::HvResult;
use crate::memory::addr::PhysAddr;
use crate::percpu::PerCpu;

numeric_enum! {
    #[repr(u32)]
    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    pub enum HyperCallCode {
        HypervisorDisable = 0,
        RtStart = 1,
        RtShutdown = 2,
    }
}

impl HyperCallCode {
    fn is_privileged(self) -> bool {
        (self as u32).get_bits(30..32) == 0
    }
}

pub type HyperCallResult = HvResult<usize>;

pub struct HyperCall<'a> {
    cpu_data: &'a mut PerCpu,
    _gpt: GuestPageTableImmut,
}

impl<'a> HyperCall<'a> {
    pub fn new(cpu_data: &'a mut PerCpu) -> Self {
        Self {
            _gpt: cpu_data.vcpu.guest_page_table(),
            cpu_data,
        }
    }

    pub fn hypercall(&mut self, code: u32, arg0: u64, _arg1: u64) -> HvResult {
        let code = match HyperCallCode::try_from(code) {
            Ok(code) => code,
            Err(_) => {
                warn!("Hypercall not supported: {}", code);
                return Ok(());
            }
        };

        if self.cpu_data.vcpu.guest_is_privileged() {
            if !code.is_privileged() {
                warn!("Cannot call {:?} in privileged mode", code);
                self.cpu_data.fault()?;
                return Ok(());
            }
        } else if code.is_privileged() {
            warn!("Cannot call {:?} in non-privileged mode", code);
            self.cpu_data.fault()?;
            return Ok(());
        }

        debug!("HyperCall: {:?} => arg0={:#x}", code, arg0);
        let ret = match code {
            HyperCallCode::HypervisorDisable => self.hypervisor_disable(),
            HyperCallCode::RtStart => self.start_rtos(arg0 as _),
            HyperCallCode::RtShutdown => self.shutdown_rtos(),
        };
        if ret.is_err() {
            warn!("HyperCall: {:?} <= {:x?}", code, ret);
        } else {
            debug!("HyperCall: {:?} <= {:x?}", code, ret);
        }

        if !code.is_privileged() {
            if ret.is_err() {
                self.cpu_data.fault()?;
            }
        } else {
            let val = match ret {
                Ok(ret) => ret,
                Err(err) => err.code() as _,
            };
            self.cpu_data.vcpu.set_return_val(val);
        }

        Ok(())
    }

    fn hypervisor_disable(&mut self) -> HyperCallResult {
        let cpus = PerCpu::activated_cpus();

        static TRY_DISABLE_CPUS: AtomicU32 = AtomicU32::new(0);
        TRY_DISABLE_CPUS.fetch_add(1, Ordering::SeqCst);
        while TRY_DISABLE_CPUS.load(Ordering::Acquire) < cpus {
            core::hint::spin_loop();
        }

        self.cpu_data.deactivate_vmm(0)?;
        unreachable!()
    }

    fn start_rtos(&mut self, entry_paddr: PhysAddr) -> HyperCallResult {
        let sys_config = crate::config::HvSystemConfig::get();
        let rt_mem_start = sys_config.rtos_memory.phys_start;
        let rt_mem_end = rt_mem_start + sys_config.rtos_memory.size;
        if !(rt_mem_start..rt_mem_end).contains(&(entry_paddr as u64)) {
            return hv_result_err!(EINVAL);
        }

        info!("Starting RTOS: entry={:#x}", entry_paddr);
        unsafe { crate::arch::start_rt_cpus(entry_paddr)? };
        Ok(0)
    }

    fn shutdown_rtos(&mut self) -> HyperCallResult {
        info!("Shutting down RTOS...");
        unsafe { crate::arch::shutdown_rt_cpus()? };
        Ok(0)
    }
}
