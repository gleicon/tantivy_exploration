use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::Snippet;
use tantivy::SnippetGenerator;
use tantivy::{doc, Index};

#[derive(Deserialize)]
pub struct AutosuggestRequest {
    prefix: String,
}

#[derive(Serialize)]
pub struct AutosuggestResponse {
    suggestions: Vec<String>,
}

pub struct AppState {
    pub index: Index,
    pub query_parser: QueryParser,
}
pub async fn autosuggest(
    data: web::Data<AppState>,
    query: web::Query<AutosuggestRequest>,
) -> HttpResponse {
    let searcher = data.index.reader().unwrap().searcher();
    let query = data
        .query_parser
        .parse_query(&format!("{:?}", query.prefix))
        .unwrap();
    let top_docs = searcher.search(&query, &TopDocs::with_limit(10)).unwrap();

    let body_field = data.index.schema().get_field("body").unwrap();
    let mut suggestions = Vec::new();

    // Create a snippet generator
    let snippet_generator = SnippetGenerator::create(&searcher, &query, body_field).unwrap();

    for (_score, doc_address) in top_docs {
        let retrieved_doc: TantivyDocument = searcher.doc(doc_address).unwrap();
        let body_content = retrieved_doc
            .get_first(body_field)
            .unwrap()
            .as_str()
            .unwrap();

        // Generate snippet
        let temp_doc = doc!(body_field => body_content);

        let snippet = snippet_generator.snippet_from_doc(&temp_doc);
        let formatted_snippet = format_snippet(&snippet, 5); // 5 words before and after the highlighted term

        suggestions.push(formatted_snippet);
    }

    HttpResponse::Ok().json(AutosuggestResponse { suggestions })
}

// Helper function to format the snippet
fn format_snippet(snippet: &Snippet, context_size: usize) -> String {
    let mut result = String::new();
    let fragment = snippet.fragment();

    let words: Vec<&str> = fragment.split_whitespace().collect();

    let context: String = words
        .iter()
        .take(context_size)
        .chain(std::iter::once(&"..."))
        .chain(words.iter().rev().take(context_size).rev())
        .cloned()
        .collect();
    result.push_str(&context);

    result
}
