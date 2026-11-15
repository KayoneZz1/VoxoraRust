/// Radyal menü sayfalama matematiği (C# RadialPaging ile birebir).
pub const PAGE_SIZE: usize = 8;

pub fn page_count(item_count: usize) -> usize {
    item_count.div_ceil(PAGE_SIZE).max(1)
}

pub fn page_items<T: Clone>(items: &[T], page: usize) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }
    let pages = page_count(items.len());
    let normalized = page % pages;
    items
        .iter()
        .skip(normalized * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect()
}

pub fn next_page(page: usize, delta: i32, item_count: usize) -> usize {
    let pages = page_count(item_count) as i32;
    let value = (page as i32 + delta).rem_euclid(pages);
    value as usize
}
