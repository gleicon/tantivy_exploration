use actix_web::{web, App, HttpResponse, HttpServer};
use error_chain::error_chain;
use glob::{glob_with, MatchOptions};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{doc, Index, IndexWriter, ReloadPolicy};

error_chain! {
    foreign_links {
        Glob(glob::GlobError);
        Pattern(glob::PatternError);
    }
}

#[derive(Deserialize)]
struct AutosuggestRequest {
    prefix: String,
}

#[derive(Serialize)]
struct AutosuggestResponse {
    suggestions: Vec<String>,
}

struct AppState {
    index: Index,
    query_parser: QueryParser,
}

async fn autosuggest(
    data: web::Data<AppState>,
    query: web::Query<AutosuggestRequest>,
) -> HttpResponse {
    let searcher = data.index.reader().unwrap().searcher();
    let query = data
        .query_parser
        .parse_query(&format!("{:?}", query.prefix))
        .unwrap();
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10)).unwrap();

    let term_field = data.index.schema().get_field("body").unwrap();
    let mut suggestions = Vec::new();
    let body_field = data.index.schema().get_field("body").unwrap();
    for (_score, doc_address) in top_docs {
        // let retrieved_doc: String = searcher.doc(doc_address).unwrap();
        // if let Some(term) = retrieved_doc.get_first(term_field) {
        //     suggestions.push(term.as_text().unwrap().to_string());
        // }
        //if let Some((_score, doc_address)) = &top_docs.into_iter().next() {
        let explanation = query.explain(&searcher, doc_address.clone());
        println!("{}", explanation.unwrap().to_pretty_json());
        suggestions.push(format!(
            "{:?}",
            searcher
                .doc::<TantivyDocument>(doc_address)
                .unwrap()
                .get_first(body_field)
                .unwrap()
                .as_str(),
        ));
    }

    HttpResponse::Ok().json(AutosuggestResponse { suggestions })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let index_path = PathBuf::from("./tantivy_pdf_index");
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("body", TEXT | STORED);
    let schema = schema_builder.build();

    // Check if the index already exists
    let index = if Path::new(&index_path).exists() {
        println!("Index already exists, opening existing index");
        Index::open_in_dir(&index_path).unwrap()
    } else {
        println!("Index does not exist, creating new index");
        fs::create_dir(index_path.clone())?;
        Index::create_in_dir(&index_path, schema.clone()).unwrap()
    };

    let mut index_writer: IndexWriter = index.writer(50_000_000).unwrap();

    let title = schema.get_field("title").unwrap();
    let body = schema.get_field("body").unwrap();
    // Check if the index is empty
    let reader = index.reader().unwrap();
    let searcher = reader.searcher();

    if searcher.num_docs() == 0 {
        println!("Index is empty. Indexing PDFs...");
        let _ = index_all_pdfs(title, body, &mut index_writer);
        index_writer.commit().unwrap();
    } else {
        println!("Index already contains {} documents", searcher.num_docs());
    }

    // Only index PDFs if the index is newly created

    println!("Searching");
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommitWithDelay)
        .try_into()
        .unwrap();

    let searcher = reader.searcher();

    let query_parser = QueryParser::for_index(&index, vec![title, body]);

    // let query = query_parser.parse_query("linux")?;

    // let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;
    // for (_score, doc_address) in top_docs {
    //     let _retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
    //     //   println!("{:<10}", retrieved_doc.to_json(&schema));
    // }

    //let query = query_parser.parse_query("title:timeseries^20 body:median^70")?;

    // let search_results = searcher.search(&query, &TopDocs::with_limit(1))?;

    // if let Some((_score, doc_address)) = search_results.into_iter().next() {
    //     let explanation = query.explain(&searcher, doc_address)?;
    //     println!("{}", explanation.to_pretty_json());
    // } else {
    //     println!("No results found for the given query.");
    // }

    let app_state = web::Data::new(AppState {
        index,
        query_parser,
    });

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/autosuggest", web::get().to(autosuggest))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
    //    Ok(())
}

fn index_all_pdfs(
    title: tantivy::schema::Field,
    body: tantivy::schema::Field,
    index_writer: &mut tantivy::IndexWriter,
) -> Result<String> {
    let options = MatchOptions {
        case_sensitive: false,
        ..Default::default()
    };

    for entry in glob_with("/Users/gleicon/Downloads/*.pdf", options)? {
        match entry {
            Ok(pb) => {
                println!("parsing {}", pb.clone().display());
                match parse_pdf(pb.clone()) {
                    Ok(out) => {
                        //println!("out: {:?}", out);
                        let mut le_doc = TantivyDocument::default();
                        le_doc.add_text(title, pb.clone().display());

                        le_doc.add_text(body, out.join(" "));
                        let _ = index_writer.add_document(le_doc);

                        let _ = index_writer.commit();
                        //return Ok("Parsed".into());
                    }
                    Err(e) => {
                        println!("Error Parsing {:?}", e);
                        //return Err(e);
                    }
                }
                // parse
            }
            Err(_e) => {
                //println!("Error {}", e);
                ()
            }
        }
    }

    return Ok("Done".into());
}

fn parse_pdf(file: PathBuf) -> Result<Vec<String>> {
    match lopdf::Document::load(file.clone()) {
        Ok(document) => {
            let pages = document.get_pages();
            let mut texts = Vec::new();

            for (i, _) in pages.iter().enumerate() {
                let page_number = (i + 1) as u32;
                let text = document.extract_text(&[page_number]);
                texts.push(text.unwrap_or_default());
            }

            //println!("Text on page {}: {}", 42, texts[41]);
            Ok(texts)
        }
        Err(e) => {
            println!("Error: {}", e);
            return Err("erro".into());
        }
    }
}
