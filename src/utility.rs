use config::SEPARATOR;

pub fn get_first_arg(a: Vec<String>) -> Option<i32> {
	if a.len() != 2 {
		None
	} else {
		match a[1].parse::<i32>() {
			Ok(x) => Some(x),
			Err(_) => None
		}
	}
}
pub fn chars_to_string(chars: &[char]) -> String {
	let mut s = String::new();
	for c in chars.iter() {
		if *c == ' ' { break; }
		s = s + &c.to_string();
	}
	s
}
pub fn int_to_mask(i: u8) -> Option<u16> {
    if i > 15 {
        None
    } else {
        let mask: u16 = (1 as u16).rotate_left(i as u32);
        Some(mask)
    }
}
