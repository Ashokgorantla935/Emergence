use image::GenericImageView;
fn main() {
    let img = image::open("../../assets/sprites/190_assets/flora_spritesheet_190.png").unwrap();
    let (w, h) = img.dimensions();
    println!("Image {}x{}", w, h);
    let mut cols = vec![];
    for x in 0..w {
        let mut is_magenta = true;
        for y in 0..h {
            let p = img.get_pixel(x, y);
            // threshold for jpeg magenta
            if p[0] < 200 || p[1] > 50 || p[2] < 200 { 
                is_magenta = false;
                break;
            }
        }
        if is_magenta { cols.push(x); }
    }
    println!("Magenta cols count: {}", cols.len());
    let mut runs = vec![];
    let mut current_run = 0;
    for x in 0..w {
        if cols.contains(&x) {
            current_run += 1;
        } else {
            if current_run > 0 { runs.push(current_run); }
            current_run = 0;
        }
    }
    if current_run > 0 { runs.push(current_run); }
    println!("Magenta column gaps: {:?}", runs);
}
