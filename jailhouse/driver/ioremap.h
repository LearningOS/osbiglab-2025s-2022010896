#include "pgalloc-track.h"

extern struct mm_struct *init_mm_sym;

extern typeof(__pte_alloc_kernel) *__pte_alloc_kernel_sym;
extern typeof(pud_free_pmd_page) *pud_free_pmd_page_sym;
extern typeof(pmd_set_huge) *pmd_set_huge_sym;
extern typeof(pud_set_huge) *pud_set_huge_sym;
extern typeof(pmd_free_pte_page) *pmd_free_pte_page_sym;

int jailhouse_ioremap_page_range(
	unsigned long addr, unsigned long end, phys_addr_t phys_addr,
	pgprot_t prot);
