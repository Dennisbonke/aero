initSidebarItems({"constant":[["IA32_APIC_BASE","APIC Location and Status (R/W)."],["IA32_EFER",""],["IA32_FMASK","System Call Flag Mask (R/W)."],["IA32_FS_BASE",""],["IA32_GS_BASE",""],["IA32_LSTAR","IA-32e Mode System Call Target Address (R/W)."],["IA32_STAR","System Call Target Address (R/W)."]],"fn":[["delay",""],["inb","Wrapper function to the `inb` assembly instruction used to do the 8-bit low level port input."],["inl","Wrapper function to the `inl` assembly instruction used to do the 32-bit low level port input."],["inw","Wrapper function to the `inw` assembly instruction used to do the 16-bit low level port input."],["outb","Wrapper function to the `outb` assembly instruction used to do the 8-bit low level port output."],["outl","Wrapper function to the `outl` assembly instruction used to do the low level port output."],["outw","Wrapper function to the `outw` assembly instruction used to do the 16-bit low level port output."],["rdmsr","Wrapper function to the `rdmsr` assembly instruction used"],["wait","This function is called after every `outb` and `outl` instruction as on older machines its necessary to give the PIC some time to react to commands as they might not be processed quickly."],["wrmsr","Wrapper function to the `wrmsr` assembly instruction used to write 64 bits to msr register."]],"struct":[["BasedPort",""]],"trait":[["InOut",""]]});