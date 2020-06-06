ENTRY(_entry)

SECTIONS
{
	.multiboot :
	{
		*(.multiboot)
	}

	. = 1M;

	.text : ALIGN(4K)
	{
		text_start = .;
		*(.text)
	}

	.rodata : ALIGN(4K)
	{
		*(.rodata)
	}

	.data : ALIGN(4K)
	{
		*(.data)
		data_end = .;
	}

	.bss : ALIGN(4K)
	{
		*(COMMON)
		*(.bss)
		bss_end = .;
	}

	kernel_end = .;
}
