//! A small writer for OLE compound files (MS-CFB version 3, 512-byte
//! sectors): one root storage holding a few streams. DocuWorks keeps an
//! embedded OLE object as such a file (see `edit::ole_picture`).

const FREE: u32 = 0xffff_ffff;
const END: u32 = 0xffff_fffe;
const FATSECT: u32 = 0xffff_fffd;
const DIFSECT: u32 = 0xffff_fffc;
const NONE: u32 = 0xffff_ffff;
const SECTOR: usize = 512;
const MINI: usize = 64;
const CUTOFF: usize = 4096;

fn sectors(n: usize, size: usize) -> usize {
    n.div_ceil(size)
}

/// Order of names in a storage: shorter first, then by upper case.
fn name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let (ua, ub): (Vec<u16>, Vec<u16>) = (a.to_uppercase().encode_utf16().collect(), b.to_uppercase().encode_utf16().collect());
    ua.len().cmp(&ub.len()).then(ua.cmp(&ub))
}

struct Node {
    left: u32,
    right: u32,
    red: bool,
}

/// A balanced tree over the sorted entries `ids` (directory numbers); the
/// deepest level is red, which keeps every path equally black.
fn tree(ids: &[u32], depth: usize, max: usize, nodes: &mut Vec<(u32, Node)>) -> u32 {
    if ids.is_empty() {
        return NONE;
    }
    let mid = ids.len() / 2;
    let left = tree(&ids[..mid], depth + 1, max, nodes);
    let right = tree(&ids[mid + 1..], depth + 1, max, nodes);
    nodes.push((ids[mid], Node { left, right, red: depth == max && depth > 0 }));
    ids[mid]
}

fn depth_of(n: usize) -> usize {
    if n <= 1 {
        0
    } else {
        1 + depth_of(n / 2).max(depth_of(n - n / 2 - 1))
    }
}

#[allow(clippy::too_many_arguments)]
fn dir_entry(name: &str, kind: u8, black: bool, left: u32, right: u32, child: u32, clsid: &[u8; 16], start: u32, size: u64) -> [u8; 128] {
    let mut e = [0u8; 128];
    if !name.is_empty() {
        let u: Vec<u16> = name.encode_utf16().collect();
        for (k, c) in u.iter().take(31).enumerate() {
            e[2 * k..2 * k + 2].copy_from_slice(&c.to_le_bytes());
        }
        e[64..66].copy_from_slice(&(((u.len().min(31) + 1) * 2) as u16).to_le_bytes());
    }
    e[66] = kind;
    e[67] = black as u8;
    e[68..72].copy_from_slice(&left.to_le_bytes());
    e[72..76].copy_from_slice(&right.to_le_bytes());
    e[76..80].copy_from_slice(&child.to_le_bytes());
    e[80..96].copy_from_slice(clsid);
    e[116..120].copy_from_slice(&start.to_le_bytes());
    e[120..128].copy_from_slice(&size.to_le_bytes());
    e
}

/// A compound file whose root storage has class `clsid` and holds
/// `streams` (name, data).
pub fn write(clsid: [u8; 16], streams: &[(&str, &[u8])]) -> Vec<u8> {
    // where each stream goes: big ones in sectors, small ones in the mini stream
    let mut mini = Vec::new();
    let mut mini_fat: Vec<u32> = Vec::new();
    let mut mini_start = vec![END; streams.len()];
    for (k, (_, d)) in streams.iter().enumerate() {
        if d.len() < CUTOFF && !d.is_empty() {
            let first = mini_fat.len() as u32;
            let n = sectors(d.len(), MINI);
            for j in 0..n {
                mini_fat.push(if j + 1 == n { END } else { first + j as u32 + 1 });
            }
            mini_start[k] = first;
            mini.extend_from_slice(d);
            mini.resize(mini_fat.len() * MINI, 0);
        }
    }
    // sector runs in file order: big streams, mini stream, mini FAT, directory
    let mut runs: Vec<usize> = Vec::new();
    for (_, d) in streams {
        runs.push(if d.len() >= CUTOFF { sectors(d.len(), SECTOR) } else { 0 });
    }
    let n_mini = sectors(mini.len(), SECTOR);
    let n_minifat = sectors(mini_fat.len() * 4, SECTOR);
    let n_dir = sectors((streams.len() + 1) * 128, SECTOR);
    let data: usize = runs.iter().sum::<usize>() + n_mini + n_minifat + n_dir;
    // FAT and DIFAT sectors needed to describe everything, themselves included
    let (mut n_fat, mut n_difat) = (1usize, 0usize);
    loop {
        let difat = if n_fat > 109 { sectors(n_fat - 109, 127) } else { 0 };
        let need = sectors(data + n_fat + difat, SECTOR / 4);
        if need <= n_fat && difat == n_difat {
            break;
        }
        n_fat = need.max(n_fat);
        n_difat = difat;
    }
    let total = data + n_fat + n_difat;
    let mut fat = vec![FREE; n_fat * SECTOR / 4];
    let mut next = 0usize;
    let mut chain = |len: usize, fat: &mut Vec<u32>| -> u32 {
        if len == 0 {
            return END;
        }
        let first = next;
        for j in 0..len {
            fat[first + j] = if j + 1 == len { END } else { (first + j + 1) as u32 };
        }
        next += len;
        first as u32
    };
    let big_start: Vec<u32> = runs.iter().map(|&n| chain(n, &mut fat)).collect();
    let mini_sect = chain(n_mini, &mut fat);
    let minifat_sect = chain(n_minifat, &mut fat);
    let dir_sect = chain(n_dir, &mut fat);
    let fat_first = next;
    for k in 0..n_fat {
        fat[fat_first + k] = FATSECT;
    }
    let difat_first = fat_first + n_fat;
    for k in 0..n_difat {
        fat[difat_first + k] = DIFSECT;
    }
    debug_assert_eq!(difat_first + n_difat, total);

    // directory: root, then the streams; the streams form one tree
    let mut ids: Vec<u32> = (1..=streams.len() as u32).collect();
    ids.sort_by(|&a, &b| name_cmp(streams[a as usize - 1].0, streams[b as usize - 1].0));
    let mut nodes = Vec::new();
    let top = tree(&ids, 0, depth_of(ids.len()), &mut nodes);
    let mut dir = Vec::with_capacity(n_dir * SECTOR);
    dir.extend_from_slice(&dir_entry("Root Entry", 5, true, NONE, NONE, top, &clsid, if mini.is_empty() { END } else { mini_sect }, mini.len() as u64));
    for (k, (name, d)) in streams.iter().enumerate() {
        let n = nodes.iter().find(|(id, _)| *id == k as u32 + 1).map(|(_, n)| n).expect("every stream is in the tree");
        let start = if d.len() >= CUTOFF { big_start[k] } else { mini_start[k] };
        dir.extend_from_slice(&dir_entry(name, 2, !n.red, n.left, n.right, NONE, &[0; 16], start, d.len() as u64));
    }
    while dir.len() < n_dir * SECTOR {
        dir.extend_from_slice(&dir_entry("", 0, false, NONE, NONE, NONE, &[0; 16], 0, 0));
    }

    let mut out = Vec::with_capacity((total + 1) * SECTOR);
    out.extend_from_slice(&[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
    out.extend_from_slice(&[0; 16]);
    for v in [0x003eu16, 3, 0xfffe, 9, 6] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[0; 6]);
    let minifat_first = if n_minifat > 0 { minifat_sect } else { END };
    let difat_start = if n_difat > 0 { difat_first as u32 } else { END };
    for v in [0u32, n_fat as u32, dir_sect, 0, CUTOFF as u32, minifat_first, n_minifat as u32, difat_start, n_difat as u32] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for k in 0..109 {
        let v = if k < n_fat { (fat_first + k) as u32 } else { FREE };
        out.extend_from_slice(&v.to_le_bytes());
    }
    debug_assert_eq!(out.len(), SECTOR);
    let pad = |out: &mut Vec<u8>| {
        while !out.len().is_multiple_of(SECTOR) {
            out.push(0);
        }
    };
    for (k, (_, d)) in streams.iter().enumerate() {
        if runs[k] > 0 {
            out.extend_from_slice(d);
            pad(&mut out);
        }
    }
    out.extend_from_slice(&mini);
    pad(&mut out);
    for v in &mini_fat {
        out.extend_from_slice(&v.to_le_bytes());
    }
    while !out.len().is_multiple_of(SECTOR) {
        out.extend_from_slice(&FREE.to_le_bytes());
    }
    out.extend_from_slice(&dir);
    for v in &fat {
        out.extend_from_slice(&v.to_le_bytes());
    }
    // DIFAT sectors: the FAT sectors after the first 109, 127 to a sector
    for k in 0..n_difat {
        for j in 0..127 {
            let f = 109 + k * 127 + j;
            let v = if f < n_fat { (fat_first + f) as u32 } else { FREE };
            out.extend_from_slice(&v.to_le_bytes());
        }
        let link = if k + 1 < n_difat { (difat_first + k + 1) as u32 } else { END };
        out.extend_from_slice(&link.to_le_bytes());
    }
    debug_assert_eq!(out.len(), (total + 1) * SECTOR);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_and_big_streams() {
        let big = vec![7u8; 5000];
        let f = write([1; 16], &[("\u{1}Ole", &[1, 0, 0, 2]), ("\u{1}CompObj", &[9; 88]), ("CONTENTS", &big)]);
        assert_eq!(&f[..8], &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1]);
        assert_eq!(f.len() % SECTOR, 0);
        // CONTENTS starts at sector 0, right after the header
        assert_eq!(&f[SECTOR..SECTOR + 4], &[7, 7, 7, 7]);
    }

    #[test]
    fn tree_is_balanced() {
        assert_eq!(depth_of(1), 0);
        assert_eq!(depth_of(3), 1);
        assert_eq!(depth_of(4), 2);
    }
}
