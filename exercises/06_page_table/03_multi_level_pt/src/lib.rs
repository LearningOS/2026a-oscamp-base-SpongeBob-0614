//! # SV39 三级页表
//!
//! 本练习模拟 RISC-V SV39 三级页表的构造和地址翻译。
//! 注意，实际上的三级页表实现并非如本练习中使用 HashMap 模拟，本练习仅作为模拟帮助学习。
//! 你需要实现页表的创建、映射和地址翻译（页表遍历）。
//!
//! ## 知识点
//! - SV39：39 位虚拟地址，三级页表
//! - VPN 拆分：VPN[2] (9bit) | VPN[1] (9bit) | VPN[0] (9bit)
//! - 页表遍历（page table walk）逐级查找
//! - 大页（2MB superpage）映射
//!
//! ## SV39 虚拟地址布局
//! ```text
//! 38        30 29       21 20       12 11        0
//! ┌──────────┬───────────┬───────────┬───────────┐
//! │ VPN[2]   │  VPN[1]   │  VPN[0]   │  offset   │
//! │  9 bits  │  9 bits   │  9 bits   │  12 bits  │
//! └──────────┴───────────┴───────────┴───────────┘
//! ```

use std::collections::HashMap;

/// 页大小 4KB
pub const PAGE_SIZE: usize = 4096;
/// 每级页表有 512 个条目 (2^9)
pub const PT_ENTRIES: usize = 512;

/// PTE 标志位
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;

/// PPN 在 PTE 中的偏移
const PPN_SHIFT: u32 = 10;
const PPN_MASK: u64 = (1 << 44) - 1;
const LEAF_FLAGS: u64 = PTE_R | PTE_W | PTE_X;

/// 页表节点：一个包含 512 个条目的数组
#[derive(Clone)]
pub struct PageTableNode {
    pub entries: [u64; PT_ENTRIES],
}

impl PageTableNode {
    pub fn new() -> Self {
        Self {
            entries: [0; PT_ENTRIES],
        }
    }
}

impl Default for PageTableNode {
    fn default() -> Self {
        Self::new()
    }
}

/// 模拟的三级页表。
///
/// 使用 HashMap<u64, PageTableNode> 模拟物理内存中的页表页。
/// `root_ppn` 是根页表所在的物理页号。
pub struct Sv39PageTable {
    /// 物理页号 -> 页表节点
    nodes: HashMap<u64, PageTableNode>,
    /// 根页表的物理页号
    pub root_ppn: u64,
    /// 下一个可分配的物理页号（简易分配器）
    next_ppn: u64,
}

/// 翻译结果
#[derive(Debug, PartialEq)]
pub enum TranslateResult {
    Ok(u64),
    PageFault,
}

impl Sv39PageTable {
    pub fn new() -> Self {
        let mut pt = Self {
            nodes: HashMap::new(),
            root_ppn: 0x80000,
            next_ppn: 0x80001,
        };
        pt.nodes.insert(pt.root_ppn, PageTableNode::new());
        pt
    }

    /// 分配一个新的物理页并初始化为空页表节点，返回其 PPN。
    fn alloc_node(&mut self) -> u64 {
        let ppn = self.next_ppn;
        self.next_ppn += 1;
        self.nodes.insert(ppn, PageTableNode::new());
        ppn
    }

    /// 从 39 位虚拟地址中提取第 `level` 级的 VPN。
    ///
    /// - level=2: 取 bits [38:30]
    /// - level=1: 取 bits [29:21]
    /// - level=0: 取 bits [20:12]
    ///
    /// 提示：右移 (12 + level * 9) 位，然后与 0x1FF 做掩码。
    pub fn extract_vpn(va: u64, level: usize) -> usize {
        assert!(level < 3, "SV39 has only three page-table levels");
        ((va >> (12 + level * 9)) & 0x1ff) as usize
    }

    /// Return an existing child table, allocating it if the entry is invalid.
    fn child_table(&mut self, parent: u64, index: usize) -> u64 {
        let entry = self.nodes[&parent].entries[index];
        if entry & PTE_V != 0 {
            assert_eq!(
                entry & LEAF_FLAGS,
                0,
                "mapping overlaps an existing superpage"
            );
            let child = (entry >> PPN_SHIFT) & PPN_MASK;
            assert!(self.nodes.contains_key(&child), "missing page-table node");
            child
        } else {
            let child = self.alloc_node();
            self.nodes.get_mut(&parent).unwrap().entries[index] = (child << PPN_SHIFT) | PTE_V;
            child
        }
    }

    /// 建立从虚拟页到物理页的映射（4KB 页）。
    ///
    /// 参数：
    /// - `va`: 虚拟地址（会自动对齐到页边界）
    /// - `pa`: 物理地址（会自动对齐到页边界）
    /// - `flags`: 标志位（如 PTE_V | PTE_R | PTE_W）
    pub fn map_page(&mut self, va: u64, pa: u64, flags: u64) {
        let mut node_ppn = self.root_ppn;
        for level in (1..=2).rev() {
            node_ppn = self.child_table(node_ppn, Self::extract_vpn(va, level));
        }
        self.nodes.get_mut(&node_ppn).unwrap().entries[Self::extract_vpn(va, 0)] =
            (((pa >> 12) & PPN_MASK) << PPN_SHIFT) | (flags & 0x3ff);
    }

    /// 遍历三级页表，将虚拟地址翻译为物理地址。
    ///
    /// 步骤：
    /// 1. 从根页表（root_ppn）开始
    /// 2. 对每一级（2, 1, 0）：
    ///    a. 用 VPN[level] 索引当前页表节点
    ///    b. 如果 PTE 无效（!PTE_V），返回 PageFault
    ///    c. 如果 PTE 是叶节点（R|W|X 有任一置位），提取 PPN 计算物理地址
    ///    d. 否则用 PTE 中的 PPN 进入下一级页表
    /// 3. level 0 的 PTE 必须是叶节点
    pub fn translate(&self, va: u64) -> TranslateResult {
        let mut node_ppn = self.root_ppn;
        for level in (0..=2).rev() {
            let Some(node) = self.nodes.get(&node_ppn) else {
                return TranslateResult::PageFault;
            };
            let entry = node.entries[Self::extract_vpn(va, level)];
            if entry & PTE_V == 0 {
                return TranslateResult::PageFault;
            }
            let ppn = (entry >> PPN_SHIFT) & PPN_MASK;
            if entry & LEAF_FLAGS != 0 {
                let offset_mask = (1u64 << (12 + level * 9)) - 1;
                let base = ppn << 12;
                if base & offset_mask != 0 {
                    return TranslateResult::PageFault;
                }
                return TranslateResult::Ok(base | (va & offset_mask));
            }
            node_ppn = ppn;
        }
        TranslateResult::PageFault
    }

    /// 建立大页映射（2MB superpage，在 level 1 设叶子 PTE）。
    ///
    /// 2MB = 512 × 4KB，对齐要求：va 和 pa 都必须 2MB 对齐。
    ///
    /// 与 map_page 类似，但只遍历到 level 1 就写入叶子 PTE。
    pub fn map_superpage(&mut self, va: u64, pa: u64, flags: u64) {
        let mega_size: u64 = (PAGE_SIZE * PT_ENTRIES) as u64; // 2MB
        assert_eq!(va % mega_size, 0, "va must be 2MB-aligned");
        assert_eq!(pa % mega_size, 0, "pa must be 2MB-aligned");

        let child = self.child_table(self.root_ppn, Self::extract_vpn(va, 2));
        self.nodes.get_mut(&child).unwrap().entries[Self::extract_vpn(va, 1)] =
            (((pa >> 12) & PPN_MASK) << PPN_SHIFT) | (flags & 0x3ff);
    }
}

impl Default for Sv39PageTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superpage_last_byte_and_boundary() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0x200000, 0x80200000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x3fffff), TranslateResult::Ok(0x803fffff));
        assert_eq!(pt.translate(0x400000), TranslateResult::PageFault);
    }

    #[test]
    fn test_invalid_nonleaf_and_missing_nodes_fault() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1234, 0x8000abcd, PTE_V);
        assert_eq!(pt.translate(0x1234), TranslateResult::PageFault);
        pt.map_page(0x1234, 0x8000abcd, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1234), TranslateResult::Ok(0x8000a234));
        pt.root_ppn = 0;
        assert_eq!(pt.translate(0x1234), TranslateResult::PageFault);
    }

    #[test]
    #[should_panic(expected = "overlaps an existing superpage")]
    fn test_page_inside_superpage_is_rejected() {
        let mut pt = Sv39PageTable::new();
        pt.map_superpage(0, 0x80000000, PTE_V | PTE_R);
        pt.map_page(0x1000, 0x90000000, PTE_V | PTE_R);
    }

    #[test]
    fn test_extract_vpn() {
        // VA = 0x0000_003F_FFFF_F000 (最大的 39 位地址的页边界)
        // VPN[2] = 0xFF (bits 38:30)
        // VPN[1] = 0x1FF (bits 29:21)
        // VPN[0] = 0x1FF (bits 20:12)
        let va: u64 = 0x7FFFFFF000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0x1FF);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0x1FF);
    }

    #[test]
    fn test_extract_vpn_simple() {
        // VA = 0x00000000 + page 1 = 0x1000
        // VPN[2] = 0, VPN[1] = 0, VPN[0] = 1
        let va: u64 = 0x1000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 1);
    }

    #[test]
    fn test_extract_vpn_level2() {
        // VPN[2] = 1 means bit 30 set -> VA >= 0x40000000
        let va: u64 = 0x40000000;
        assert_eq!(Sv39PageTable::extract_vpn(va, 2), 1);
        assert_eq!(Sv39PageTable::extract_vpn(va, 1), 0);
        assert_eq!(Sv39PageTable::extract_vpn(va, 0), 0);
    }

    #[test]
    fn test_map_and_translate_single() {
        let mut pt = Sv39PageTable::new();
        // 映射：VA 0x1000 -> PA 0x80001000
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);

        let result = pt.translate(0x1000);
        assert_eq!(result, TranslateResult::Ok(0x80001000));
    }

    #[test]
    fn test_translate_with_offset() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x2000, 0x90000000, PTE_V | PTE_R | PTE_W);

        // 访问 VA 0x2ABC -> PA 应为 0x90000ABC
        let result = pt.translate(0x2ABC);
        assert_eq!(result, TranslateResult::Ok(0x90000ABC));
    }

    #[test]
    fn test_translate_page_fault() {
        let pt = Sv39PageTable::new();
        assert_eq!(pt.translate(0x1000), TranslateResult::PageFault);
    }

    #[test]
    fn test_multiple_mappings() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x0000_1000, 0x8000_1000, PTE_V | PTE_R);
        pt.map_page(0x0000_2000, 0x8000_5000, PTE_V | PTE_R | PTE_W);
        pt.map_page(0x0040_0000, 0x9000_0000, PTE_V | PTE_R);

        assert_eq!(pt.translate(0x1234), TranslateResult::Ok(0x80001234));
        assert_eq!(pt.translate(0x2000), TranslateResult::Ok(0x80005000));
        assert_eq!(pt.translate(0x400100), TranslateResult::Ok(0x90000100));
    }

    #[test]
    fn test_map_overwrite() {
        let mut pt = Sv39PageTable::new();
        pt.map_page(0x1000, 0x80001000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x80001000));

        pt.map_page(0x1000, 0x90002000, PTE_V | PTE_R);
        assert_eq!(pt.translate(0x1000), TranslateResult::Ok(0x90002000));
    }

    #[test]
    fn test_superpage_mapping() {
        let mut pt = Sv39PageTable::new();
        // 2MB 大页映射：VA 0x200000 -> PA 0x80200000
        pt.map_superpage(0x200000, 0x80200000, PTE_V | PTE_R | PTE_W);

        // 大页内不同偏移都应命中
        assert_eq!(pt.translate(0x200000), TranslateResult::Ok(0x80200000));
        assert_eq!(pt.translate(0x200ABC), TranslateResult::Ok(0x80200ABC));
        assert_eq!(pt.translate(0x2FF000), TranslateResult::Ok(0x802FF000));
    }

    #[test]
    fn test_superpage_and_normal_coexist() {
        let mut pt = Sv39PageTable::new();
        // 大页映射在第一个 2MB 区域
        pt.map_superpage(0x0, 0x80000000, PTE_V | PTE_R);
        // 普通页在不同的 VPN[2] 区域
        pt.map_page(0x40000000, 0x90001000, PTE_V | PTE_R);

        assert_eq!(pt.translate(0x100), TranslateResult::Ok(0x80000100));
        assert_eq!(pt.translate(0x40000000), TranslateResult::Ok(0x90001000));
    }
}
