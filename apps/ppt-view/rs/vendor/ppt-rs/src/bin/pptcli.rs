//! PPTX CLI - Command-line tool for creating PowerPoint presentations

use clap::Parser;
use ppt_rs::api::Presentation;
use ppt_rs::cli::{
    BatchCommand, Cli, Commands, CreateCommand, DiffCommand, ExportFormat, FromHtmlCommand,
    FromHtmlOptions, FromMarkdownCommand, InfoCommand, ValidateCommand,
};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            output,
            title,
            slides,
            template,
        } => match CreateCommand::execute(&output, title.as_deref(), slides, template.as_deref()) {
            Ok(_) => {
                println!("✓ Created presentation: {output}");
                let title = title.as_deref().unwrap_or("Presentation");
                println!("  Title: {title}");
                println!("  Slides: {slides}");
            }
            Err(e) => {
                eprintln!("✗ Error: {e}");
                std::process::exit(1);
            }
        },
        Commands::Md2Ppt {
            input,
            output,
            title,
        } => {
            // Auto-generate output if not provided
            let output_path = output.unwrap_or_else(|| {
                use std::path::Path;
                let input_path = Path::new(&input);
                if let Some(stem) = input_path.file_stem() {
                    if let Some(parent) = input_path.parent() {
                        if parent.as_os_str().is_empty() {
                            format!("{}.pptx", stem.to_string_lossy())
                        } else {
                            format!("{}/{}.pptx", parent.display(), stem.to_string_lossy())
                        }
                    } else {
                        format!("{}.pptx", stem.to_string_lossy())
                    }
                } else {
                    format!("{}.pptx", input)
                }
            });

            match FromMarkdownCommand::execute(&input, &output_path, title.as_deref()) {
                Ok(_) => {
                    println!("✓ Created presentation: {output_path}");
                    println!("  Input: {input}");
                    let title = title.as_deref().unwrap_or("Presentation from Markdown");
                    println!("  Title: {title}");
                }
                Err(e) => {
                    eprintln!("✗ Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Info { file } => match InfoCommand::execute(&file) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("✗ Error: {e}");
                std::process::exit(1);
            }
        },
        Commands::Diff { old, new, json } => match DiffCommand::execute(&old, &new, json) {
            Ok(()) => {
                if !json {
                    println!("\n✓ Semantic diff: no differences");
                }
            }
            Err(e) => {
                if !json {
                    eprintln!("✗ {e}");
                }
                std::process::exit(1);
            }
        },
        Commands::Batch {
            input_dir,
            output_dir,
            template,
        } => match BatchCommand::execute(&input_dir, &output_dir, template.as_deref()) {
            Ok(results) => {
                println!("Batch conversion complete\n");
                for r in &results {
                    println!(
                        "✓ {} → {} ({} slides, {} bytes)",
                        r.input.display(),
                        r.output.display(),
                        r.slide_count,
                        r.bytes
                    );
                }
                println!("\n✓ Generated {} presentation(s) in {output_dir}", results.len());
            }
            Err(e) => {
                eprintln!("✗ Error: {e}");
                std::process::exit(1);
            }
        },
        Commands::Validate { file, json } => match ValidateCommand::execute(&file, json) {
            Ok(_) => {
                if !json {
                    println!("\n✓ Validation completed successfully");
                }
            }
            Err(e) => {
                if !json {
                    eprintln!("✗ Error: {e}");
                }
                std::process::exit(1);
            }
        },
        Commands::Export {
            input,
            output,
            format,
            group_by_sections,
        } => {
            println!("Exporting {}...", input);
            let pres = match Presentation::from_path(&input) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("✗ Error loading presentation: {}", e);
                    std::process::exit(1);
                }
            };

            let format = format
                .or_else(|| {
                    let out_lower = output.to_lowercase();
                    if out_lower.ends_with(".pdf") {
                        Some(ExportFormat::Pdf)
                    } else if out_lower.ends_with(".html") {
                        Some(ExportFormat::Html)
                    } else if out_lower.ends_with(".md") || out_lower.ends_with(".markdown") {
                        Some(ExportFormat::Md)
                    } else if out_lower.ends_with(".png") {
                        Some(ExportFormat::Png)
                    } else {
                        None
                    }
                })
                .unwrap_or(ExportFormat::Pdf);

            let result = match format {
                ExportFormat::Pdf => pres.save_as_pdf(&output),
                ExportFormat::Html => {
                    if group_by_sections {
                        pres.save_as_html_with_options(
                            &output,
                            &ppt_rs::export::html::HtmlExportOptions::new()
                                .with_section_grouping(true),
                        )
                    } else {
                        pres.save_as_html(&output)
                    }
                }
                ExportFormat::Md => {
                    if group_by_sections {
                        pres.save_as_markdown_with_options(
                            &output,
                            &ppt_rs::export::md::MarkdownOptions::new()
                                .with_section_grouping(true),
                        )
                    } else {
                        pres.save_as_markdown(&output)
                    }
                }
                ExportFormat::Png => pres.save_as_png(&output),
            };

            match result {
                Ok(_) => println!("✓ Export completed: {}", output),
                Err(e) => {
                    eprintln!("✗ Error exporting: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Merge { output, inputs } => {
            println!("Merging {} files into {}...", inputs.len(), output);
            let mut final_pres = match Presentation::from_path(&inputs[0]) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("✗ Error loading {}: {}", inputs[0], e);
                    std::process::exit(1);
                }
            };

            for input in inputs.iter().skip(1) {
                match Presentation::from_path(input) {
                    Ok(p) => {
                        final_pres = final_pres.add_presentation(p);
                    }
                    Err(e) => {
                        eprintln!("✗ Error loading {}: {}", input, e);
                        std::process::exit(1);
                    }
                }
            }

            match final_pres.save(&output) {
                Ok(_) => println!("✓ Merge completed: {}", output),
                Err(e) => {
                    eprintln!("✗ Error saving merged file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Html2Ppt {
            input,
            output,
            title,
            max_slides,
            max_bullets,
            no_images,
            no_tables,
            no_code,
        } => {
            // Auto-generate output if not provided
            let output_path = output.unwrap_or_else(|| {
                use std::path::Path;
                let input_path = Path::new(&input);
                if let Some(stem) = input_path.file_stem() {
                    if let Some(parent) = input_path.parent() {
                        if parent.as_os_str().is_empty() {
                            format!("{}.pptx", stem.to_string_lossy())
                        } else {
                            format!("{}/{}.pptx", parent.display(), stem.to_string_lossy())
                        }
                    } else {
                        format!("{}.pptx", stem.to_string_lossy())
                    }
                } else {
                    format!("{}.pptx", input)
                }
            });

            match FromHtmlCommand::execute(FromHtmlOptions {
                input: &input,
                output: &output_path,
                title: title.as_deref(),
                max_slides,
                max_bullets,
                no_images,
                no_tables,
                no_code,
            }) {
                Ok(_) => {
                    println!("✓ Created presentation: {output_path}");
                    println!("  Input: {input}");
                    let title = title.as_deref().unwrap_or("Presentation from HTML");
                    println!("  Title: {title}");
                }
                Err(e) => {
                    eprintln!("✗ Error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Commands::Pdf2Ppt { input, output } => {
            let output = output.unwrap_or_else(|| {
                let path = std::path::Path::new(&input);
                path.with_extension("pptx").to_string_lossy().to_string()
            });

            println!("Converting {} to {}...", input, output);
            match Presentation::from_pdf(&input) {
                Ok(p) => match p.save(&output) {
                    Ok(_) => println!("✓ Conversion completed: {}", output),
                    Err(e) => {
                        eprintln!("✗ Error saving presentation: {}", e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("✗ Error converting PDF: {}", e);
                    std::process::exit(1);
                }
            }
        }
        #[cfg(feature = "web2ppt")]
        Commands::Web2Ppt {
            url,
            output,
            title,
            max_slides,
            max_bullets,
            no_images,
            no_tables,
            no_code,
            no_source_url,
            timeout,
            verbose,
            #[cfg(feature = "web2md")]
            web2md,
        } => {
            execute_web2ppt(Web2PptArgs {
                url,
                output,
                title,
                max_slides,
                max_bullets,
                no_images,
                no_tables,
                no_code,
                no_source_url,
                timeout,
                verbose,
                #[cfg(feature = "web2md")]
                web2md,
            });
        }
    }
}

#[cfg(feature = "web2ppt")]
struct Web2PptArgs {
    url: String,
    output: String,
    title: Option<String>,
    max_slides: usize,
    max_bullets: usize,
    no_images: bool,
    no_tables: bool,
    no_code: bool,
    no_source_url: bool,
    timeout: u64,
    verbose: bool,
    #[cfg(feature = "web2md")]
    web2md: bool,
}

#[cfg(feature = "web2ppt")]
fn execute_web2ppt(args: Web2PptArgs) {
    let Web2PptArgs {
        url,
        output,
        title,
        max_slides,
        max_bullets,
        no_images,
        no_tables,
        no_code,
        no_source_url,
        timeout,
        verbose,
        #[cfg(feature = "web2md")]
        web2md: use_web2md,
    } = args;
    use ppt_rs::{ConversionOptions, Web2Ppt, Web2PptConfig, WebFetcher, WebParser};

    if verbose {
        println!("🌐 Web2PPT - Converting webpage to PowerPoint");
        println!("   URL: {}", url);
        println!("   Output: {}", output);
        println!("   Max slides: {}", max_slides);
        println!("   Max bullets: {}", max_bullets);
        println!("   Include images: {}", !no_images);
        println!("   Include tables: {}", !no_tables);
        println!("   Include code: {}", !no_code);
        println!();
        println!("📥 Fetching webpage...");
    }

    // Build config
    let config = Web2PptConfig::new()
        .max_slides(max_slides)
        .max_bullets(max_bullets)
        .with_images(!no_images)
        .with_tables(!no_tables)
        .with_code(!no_code)
        .timeout(timeout);

    // Build options
    let mut options = ConversionOptions::new().with_source_url(!no_source_url);

    if let Some(t) = title.as_ref() {
        options = options.title(t);
    }

    // Fetch
    let fetcher = match WebFetcher::with_config(config.clone()) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("✗ Failed to create fetcher: {}", e);
            std::process::exit(1);
        }
    };

    let html = match fetcher.fetch(&url) {
        Ok(h) => {
            if verbose {
                println!("   Fetched {} bytes of HTML", h.len());
            }
            h
        }
        Err(e) => {
            eprintln!("✗ Failed to fetch: {}", e);
            std::process::exit(1);
        }
    };

    // --- web2md pipeline (HTML → Markdown → Slides) ---
    #[cfg(feature = "web2md")]
    if use_web2md {
        if verbose {
            println!("🔄 Using web2md pipeline (HTML → Markdown → Slides)");
        }
        match ppt_rs::html_to_pptx_via_web2md(&html, &url, config, &options) {
            Ok(pptx_data) => {
                if verbose {
                    println!("✅ Conversion successful!");
                    println!("   Size: {} bytes", pptx_data.len());
                    println!("💾 Saving to {}...", output);
                }
                match std::fs::write(&output, &pptx_data) {
                    Ok(_) => {
                        println!("✓ Created: {} ({} bytes)", output, pptx_data.len());
                    }
                    Err(e) => {
                        eprintln!("✗ Failed to save file: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("✗ Conversion failed: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // --- Default scraper-based pipeline ---
    // Parse
    let parser = WebParser::with_config(config.clone());
    let content = match parser.parse(&html, &url) {
        Ok(c) => {
            if verbose {
                println!("📄 Parsed content:");
                println!("   Title: {}", c.title);
                println!(
                    "   Description: {}",
                    c.description.as_deref().unwrap_or("(none)")
                );
                println!("   Content blocks: {}", c.blocks.len());
                println!("   Images: {}", c.images.len());

                // Show headings found
                let headings: Vec<_> = c
                    .blocks
                    .iter()
                    .filter(|b| b.is_heading())
                    .map(|b| b.text.as_str())
                    .take(10)
                    .collect();
                if !headings.is_empty() {
                    println!("   Headings found: {:?}", headings);
                }
            }
            c
        }
        Err(e) => {
            eprintln!("✗ Failed to parse: {}", e);
            std::process::exit(1);
        }
    };

    // Convert
    let converter = Web2Ppt::with_config(config);
    match converter.convert(&content, &options) {
        Ok(pptx_data) => {
            if verbose {
                println!("✅ Conversion successful!");
                println!("   Size: {} bytes", pptx_data.len());
                println!("💾 Saving to {}...", output);
            }

            match std::fs::write(&output, &pptx_data) {
                Ok(_) => {
                    println!("✓ Created: {} ({} bytes)", output, pptx_data.len());
                }
                Err(e) => {
                    eprintln!("✗ Failed to save file: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("✗ Conversion failed: {}", e);
            std::process::exit(1);
        }
    }
}
