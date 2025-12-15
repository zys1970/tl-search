use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use jieba_rs::Jieba;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::Cursor;

static JIEBA: Lazy<Jieba> = Lazy::new(|| {
    let mut j = Jieba::empty(); // 不加载默认词库

    // 将词库编译进 wasm
    let dict_data = include_bytes!("../assets/dict.small.txt");
    let mut cursor = Cursor::new(dict_data);

    j.load_dict(&mut cursor).unwrap();
    j
});

#[derive(Serialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub score: f64,
    pub positions: Vec<usize>,
}

#[wasm_bindgen]
pub struct TlSearch {
    documents: HashMap<String, Document>,
    inverted_index: HashMap<String, Vec<Posting>>,
    doc_count: usize,
    title_suggest: HashSet<String>,
}

struct Document {
    title: String,
    body: String,
    terms: HashSet<String>,
}

struct Posting {
    doc_id: String,
    tf: f64,  // term frequency
    positions: Vec<usize>,
}

#[wasm_bindgen]
impl TlSearch {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TlSearch {
        TlSearch {
            documents: HashMap::new(),
            inverted_index: HashMap::new(),
            doc_count: 0,
            title_suggest: HashSet::new(),
        }
    }

    #[wasm_bindgen]
    pub fn add(&mut self, id: String, title: String, body: String) {
        let mut terms = HashSet::new();
        let mut term_freq: HashMap<String, usize> = HashMap::new();
        let mut term_positions: HashMap<String, Vec<usize>> = HashMap::new(); // 新增：记录每个词在 body 中的起始位置

        // 先处理标题（标题不记录位置，只用于加权和建议）
        for word in JIEBA.cut(&title, false) {
            let w = word.to_lowercase();
            if w.len() > 1 && !STOP_WORDS.contains(&w.as_str()) {
                terms.insert(w.clone());
                *term_freq.entry(w.clone()).or_insert(0) += 1;
            }
        }

        // 处理正文：分词 + 统计词频 + 记录位置
        let mut body_offset = 0; // 用于计算字符偏移（处理中文正确）
        let body_chars: Vec<char> = body.chars().collect();

        for word in JIEBA.cut(&body, false) {
            let original_word = word; // 原始词（带大小写）
            let w = word.to_lowercase();

            if w.len() > 1 && !STOP_WORDS.contains(&w.as_str()) {
                terms.insert(w.clone());
                *term_freq.entry(w.clone()).or_insert(0) += 1;

                // 查找这个词在剩余文本中的位置
                if let Some(pos) = body[body_offset..].find(original_word) {
                    // 计算字符级别的起始位置（重要！中文不能用字节索引）
                    let char_start = body[..body_offset + pos].chars().count();
                    term_positions.entry(w.clone()).or_default().push(char_start);

                    // 更新偏移，跳过当前词，继续找下一个
                    body_offset += pos + original_word.len();
                }
            } else {
                // 停用词或太短，也要跳过
                if let Some(pos) = body[body_offset..].find(original_word) {
                    body_offset += pos + original_word.len();
                }
            }
        }

        // 更新倒排索引（现在 positions 有真实位置了）
        for (term, freq) in term_freq {
            let positions = term_positions.get(&term).cloned().unwrap_or_default();

            let posting = Posting {
                doc_id: id.clone(),
                tf: freq as f64 / terms.len() as f64,
                positions, // ← 现在是真实位置数组！
            };
            self.inverted_index.entry(term).or_default().push(posting);
        }

        self.documents.insert(
            id.clone(),
            Document {
                title: title.clone(),
                body,
                terms,
            },
        );
        self.title_suggest.insert(title.to_lowercase());
        self.doc_count += 1;
    }

    #[wasm_bindgen]
    pub fn remove(&mut self, id: &str) {
        if let Some(doc) = self.documents.remove(id) {
            for term in &doc.terms {
                if let Some(postings) = self.inverted_index.get_mut(term) {
                    postings.retain(|p| p.doc_id != id);
                    if postings.is_empty() {
                        self.inverted_index.remove(term);
                    }
                }
            }
            self.doc_count -= 1;
        }
    }

    #[wasm_bindgen]
    pub fn search(&self, query: &str, limit: usize) -> JsValue {
        let query_terms: Vec<String> = JIEBA.cut(query, false)
            .iter()
            .map(|s| s.to_lowercase())
            .filter(|s| s.len() > 1 && !STOP_WORDS.contains(s.as_str()))
            .collect();

        if query_terms.is_empty() {
            return to_value::<Vec<SearchResult>>(&vec![]).unwrap();
        }

        let mut scores: HashMap<String, f64> = HashMap::new();
        // 新增：收集每个文档的关键词位置
        let mut highlights_map: HashMap<String, Vec<usize>> = HashMap::new();

        for term in &query_terms {
            if let Some(postings) = self.inverted_index.get(term) {
                let idf = ((self.doc_count as f64) / (postings.len() as f64)).ln();

                for posting in postings {
                    let score_add = posting.tf * idf;
                    let entry = scores.entry(posting.doc_id.clone()).or_insert(0.0);
                    *entry += score_add;

                    // 标题命中加权（乘 2）
                    if self.documents[&posting.doc_id].title.to_lowercase().contains(term) {
                        *entry *= 2.0;
                    }

                    // 收集高亮位置（来自 posting.positions）
                    highlights_map
                        .entry(posting.doc_id.clone())
                        .or_default()
                        .extend_from_slice(&posting.positions);
                }
            }
        }

        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(id, score)| {
                let title = self.documents[&id].title.clone();
                let positions = highlights_map.get(&id).cloned().unwrap_or_default();

                // 可选：对位置去重 + 排序（推荐）
                let mut positions = positions;
                positions.sort_unstable();
                positions.dedup();

                SearchResult {
                    id,
                    title,
                    score,
                    positions, // ← 新增字段
                }
            })
            .collect();

        // 按分数降序排序
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        to_value(&results).unwrap()
    }

    #[wasm_bindgen]
    pub fn suggest(&self, prefix: &str) -> JsValue {
        let prefix = prefix.to_lowercase();
        let matches: Vec<String> = self.title_suggest.iter()
            .filter(|title| title.contains(&prefix))
            .take(10)
            .cloned()
            .collect();
        return to_value::<Vec<String>>(&matches).unwrap();
    }
}

static STOP_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        // 中文最常见停用词（精选 Top 50）
        "的", "了", "和", "是", "在", "我", "有", "之", "与", "或",
        "这", "那", "个", "你", "他", "她", "它", "我们", "他们", "一个",
        "被", "到", "就", "为", "于", "等", "从", "对", "还", "说", "也",
        "但", "而", "后", "来", "得", "中", "上", "下", "里", "很", "都",
        // 英文停用词
        "a", "an", "the", "and", "or", "but", "to", "of", "in", "on", "at",
        "for", "with", "by", "from", "as", "is", "was", "are", "were", "be",
        "this", "that", "i", "you", "he", "she", "it", "we", "they"
    ]
    .iter().cloned().collect()
});