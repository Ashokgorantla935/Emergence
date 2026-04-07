use image::GenericImageView;

#[test]
fn test_image_grid() {
    let img = image::open("../../assets/sprites/190_assets/flora_spritesheet_190.png").unwrap();
    let (w, h) = img.dimensions();
    println!("Flora: {}x{}", w, h);
    
    let mut x_sums = vec![0; w as usize];
    for x in 0..w {
        let mut magenta_count = 0;
        for y in 0..h {
            let p = img.get_pixel(x, y);
            // approximate magenta due to JPEG artifacts
            if p[0] > 200 && p[1] < 100 && p[2] > 200 {
                magenta_count += 1;
            }
        }
        x_sums[x as usize] = magenta_count;
    }
    
    // Find thick bands of magenta extending top-to-bottom
    let mut cols_with_lines = vec![];
    for x in 0..w {
        if x_sums[x as usize] > h as usize - 50 { // mostly magenta column
            cols_with_lines.push(x);
        }
    }
    let mut segments = vec![];
    let mut cur = vec![];
    for &val in &cols_with_lines {
        if cur.is_empty() {
            cur.push(val);
        } else if val == *cur.last().unwrap() + 1 {
            cur.push(val);
        } else {
            segments.push(cur.clone());
            cur.clear();
            cur.push(val);
        }
    }
    if !cur.is_empty() { segments.push(cur); }
    println!("Flora grid gaps at: {:?}", segments.iter().map(|s| s[0]).collect::<Vec<_>>());
}
