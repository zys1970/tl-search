# tl-search

**纯 Rust 手搓的超快中文全文搜索引擎**  

[![npm](https://img.shields.io/npm/v/tl-search?color=red&style=flat-square)](https://www.npmjs.com/package/tl-search)  
[![size](https://img.shields.io/bundlephobia/minzip/tl-search?style=flat-square)](https://bundlephobia.com/package/tl-search)  
[![license](https://img.shields.io/npm/l/tl-search?style=flat-square)](#license)  
![Rust](https://img.shields.io/badge/Rust-100%25-black?style=flat-square&logo=rust)

## 安装

```bash
npm install tl-search
```

## 极简使用（3 行搞定）

```js
import init, { TlSearch } from 'tl-search';

await init();
const search = new TlSearch();

// 添加文档
search.add("1", "Rust 编程语言", "Rust 是一门系统编程语言，注重安全与性能");
search.add("2", "学习 Rust 的好处", "零成本抽象、无数据竞争、并发安全");

// 搜索
const results = search.search("rust 安全", 10);
console.log(results);
// → [
//   { id: "1", title: "Rust 编程语言", score: 2.77, positions: [0, 13, 37] },
//   ...
// ]

// 前缀建议
const hints = search.suggest("ru");
console.log(hints);
 // → ["Rust 编程语言", "学习 Rust 的好处"]
```

## 核心特性

- 100% 手搓实现（无 tantivy / tinysearch / lunr）
- 完美中文分词（jieba-rs）
- TF-IDF + 标题加权排序
- 前缀自动建议（输入 `ru` → 自动提示标题）
- 支持增量添加/删除文档
- 零 native 依赖，可直接在浏览器运行

## 适用场景

- 静态博客搜索（VitePress、Docusaurus、Astro、Hexo）
- 文档站本地搜索
- 前端离线搜索
- 任何对性能极致的中文搜索需求

## License

[MIT License](LICENSE) © tl1015

---