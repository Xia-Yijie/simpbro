use simpbro::browser::Browser;

fn main() {
    let mut browser = Browser::new().expect("Failed to create browser");

    let url = std::env::args().nth(1).unwrap_or_else(|| "https://example.com".to_string());
    println!("=== Fetching {} ===\n", url);
    match browser.fetch(&url) {
        Ok(page) => {
            println!("URL:   {}", page.url);
            println!("Title: {}", page.title);
            println!("Links: {}", page.links.len());
            println!("\n--- Content ---");
            for line in &page.lines {
                match line {
                    simpbro::browser::PageLine::Heading(t, level) => {
                        println!("[H{}] {}", level, t);
                    }
                    simpbro::browser::PageLine::Text(t) => {
                        println!("     {}", t);
                    }
                    simpbro::browser::PageLine::LinkRef(t, idx) => {
                        println!("[Link#{}] {} -> {}", idx, t, page.links[*idx].url);
                    }
                    simpbro::browser::PageLine::Blank => {
                        println!();
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
