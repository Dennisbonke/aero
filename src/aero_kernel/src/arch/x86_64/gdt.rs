//! The GDT contains entries telling the CPU about memory segments.
//!
//! **Notes**: <https://wiki.osdev.org/Global_Descriptor_Table>

use core::mem;

use x86_64::VirtAddr;

const GDT_ENTRY_COUNT: usize = 4;
const GDT_LOCAL_ENTRY_COUNT: usize = 7;

static mut GDT: [GdtEntry; GDT_ENTRY_COUNT] = [
    // GDT NULL descriptor.
    GdtEntry::NULL,
    // GDT kernel code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel TLS (Thread Local Storage) descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
];

#[thread_local]
static mut LOCAL_GDT: [GdtEntry; GDT_LOCAL_ENTRY_COUNT] = [
    // GDT null descriptor.
    GdtEntry::NULL,
    // GDT kernel code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT kernel data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_0
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT user data descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT user code descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT
            | GdtAccessFlags::RING_3
            | GdtAccessFlags::SYSTEM
            | GdtAccessFlags::EXECUTABLE
            | GdtAccessFlags::PRIVILEGE,
        GdtEntryFlags::LONG_MODE,
    ),
    // GDT TSS descriptor.
    GdtEntry::new(
        GdtAccessFlags::PRESENT | GdtAccessFlags::RING_3 | GdtAccessFlags::TSS_AVAIL,
        GdtEntryFlags::NULL,
    ),
    // GDT null descriptor as the TSS should be 16 bytes long
    // and twice the normal size.
    GdtEntry::NULL,
];

#[thread_local]
static mut TSS: TssEntry = TssEntry::new();

bitflags::bitflags! {
    /// Specifies which element to load into a segment from
    /// descriptor tables (i.e., is a index to LDT or GDT table
    /// with some additional flags).
    struct SegmentSelector: u16 {
        const RPL_0 = 0b00;
        const RPL_1 = 0b01;
        const RPL_2 = 0b10;
        const RPL_3 = 0b11;
        const TI_GDT = 0 << 2;
        const TI_LDT = 1 << 2;
    }
}

struct GdtAccessFlags;

impl GdtAccessFlags {
    const NULL: u8 = 0;
    const PRESENT: u8 = 1 << 7;
    const RING_0: u8 = 0 << 5;
    const RING_3: u8 = 3 << 5;
    const SYSTEM: u8 = 1 << 4;
    const EXECUTABLE: u8 = 1 << 3;
    const PRIVILEGE: u8 = 1 << 1;
    const TSS_AVAIL: u8 = 9;
}

bitflags::bitflags! {
    struct GdtEntryFlags: u8 {
        const NULL = 0;
        const PROTECTED_MODE = 1 << 6;
        const LONG_MODE = 1 << 5;
    }
}

impl SegmentSelector {
    #[inline(always)]
    const fn new(index: u16, rpl: Self) -> Self {
        Self {
            bits: index << 3 | rpl.bits,
        }
    }
}

#[repr(C, packed)]
struct GdtDescriptor {
    /// The size of the table subtracted by 1.
    /// The size of the table is subtracted by 1 as the maximum value
    /// of `size` is 65535, while the GDT can be up to 65536 bytes.
    size: u16,
    /// The linear address of the table.
    offset: u64,
}

impl GdtDescriptor {
    /// Create a new GDT descriptor.
    #[inline]
    pub const fn new(size: u16, offset: u64) -> Self {
        Self { size, offset }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access_byte: u8,
    /// The limit high and the flags.
    ///
    /// **Note**: Four bits of the variable is the limit and rest four bits of the
    /// variable are the flags.
    limit_hi_flags: u8,
    base_hi: u8,
}

impl GdtEntry {
    const NULL: Self = Self::new(GdtAccessFlags::NULL, GdtEntryFlags::NULL);

    const fn new(access_flags: u8, entry_flags: GdtEntryFlags) -> Self {
        Self {
            limit_low: 0x00,
            base_low: 0x00,
            base_middle: 0x00,
            access_byte: access_flags,
            limit_hi_flags: entry_flags.bits() & 0xF0,
            base_hi: 0x00,
        }
    }
}

/// The Task State Segment (TSS) is a special data structure for x86 processors which holds information about a task.
///
/// **Notes**: <https://wiki.osdev.org/Task_State_Segment>
#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct TssEntry {
    reserved: u32,
    rsp: [u64; 3],
    reserved2: u64,
    ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,
    iomap_base: u16,
}

impl TssEntry {
    #[inline]
    const fn new() -> Self {
        Self {
            reserved: 0,
            rsp: [0; 3],
            reserved2: 0,
            ist: [0; 7],
            reserved3: 0,
            reserved4: 0,
            iomap_base: 0xFFFF,
        }
    }
}

/// Initialize the GDT.
pub fn init() {
    unsafe {
        let gdt_descriptor = GdtDescriptor::new(
            (mem::size_of::<[GdtEntry; GDT_ENTRY_COUNT]>() - 1) as u16,
            (&GDT as *const _) as u64,
        );

        load_gdt(&gdt_descriptor as *const _);

        // Load the GDT segments.
        load_cs(SegmentSelector::new(1, SegmentSelector::RPL_0));
        load_ds(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_es(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_fs(SegmentSelector::new(2, SegmentSelector::RPL_0));
        load_gs(SegmentSelector::new(3, SegmentSelector::RPL_0));
        load_ss(SegmentSelector::new(2, SegmentSelector::RPL_0));
    }
}

/// Initialize the local GDT.
pub fn init_local(stack_top: VirtAddr) {
    // unsafe {
    // let gdt_descriptor = GdtDescriptor::new(
    //     (mem::size_of::<[GdtEntry; GDT_LOCAL_ENTRY_COUNT]>() - 1) as u16,
    //     (&LOCAL_GDT as *const _) as u64,
    // );

    // load_gdt(&gdt_descriptor as *const _);

    // // Reload the GDT segments.
    // load_cs(SegmentSelector::new(1, SegmentSelector::RPL_0));
    // load_ds(SegmentSelector::new(2, SegmentSelector::RPL_0));
    // load_es(SegmentSelector::new(2, SegmentSelector::RPL_0));
    // load_ss(SegmentSelector::new(2, SegmentSelector::RPL_0));

    // load_tss(SegmentSelector::new(8, SegmentSelector::RPL_0));
    // }
}

#[inline(always)]
unsafe fn load_cs(selector: SegmentSelector) {
    asm!(
        "push {selector}",
        "lea {tmp}, [1f + rip]",
        "push {tmp}",
        "retfq",
        "1:",
        selector = in(reg) u64::from(selector.bits()),
        tmp = lateout(reg) _,
    );
}

#[inline(always)]
unsafe fn load_ds(selector: SegmentSelector) {
    asm!("mov ds, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_es(selector: SegmentSelector) {
    asm!("mov es, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_fs(selector: SegmentSelector) {
    asm!("mov fs, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_gs(selector: SegmentSelector) {
    asm!("mov gs, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_ss(selector: SegmentSelector) {
    asm!("mov ss, {0:x}", in(reg) selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_tss(selector: SegmentSelector) {
    asm!("ltr [rdi]", in("rdi") selector.bits(), options(nomem, nostack))
}

#[inline(always)]
unsafe fn load_gdt(gdt_descriptor: *const GdtDescriptor) {
    asm!("lgdt [rdi]", in("rdi") gdt_descriptor, options(nostack))
}