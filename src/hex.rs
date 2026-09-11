/// Print a classic hex+ASCII dump of `data[start..end]`.
pub fn hex_dump(data: &[u8], start: usize, end: usize) {
    let end = end.min(data.len());
    for offset in (start..end).step_by(16) {
        print!("{offset:04X}: ");

        for i in 0..16 {
            if let Some(byte) = data.get(offset + i) {
                print!("{byte:02X} ");
            } else {
                print!("   ");
            }
        }

        print!(" |");

        for i in 0..16 {
            if let Some(byte) = data.get(offset + i) {
                let c = if byte.is_ascii_graphic() || *byte == b' ' {
                    *byte as char
                } else {
                    '.'
                };
                print!("{c}");
            }
        }

        println!("|");
    }
}
