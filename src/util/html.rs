use scraper::{ElementRef, Html, Selector};

pub fn sel(s: &str) -> Selector {
    Selector::parse(s).unwrap_or_else(|e| panic!("invalid selector `{s}`: {e:?}"))
}

/// cheerio 的 `.text()`：拼接所有后代文本节点后 trim。
pub fn txt(el: ElementRef<'_>) -> String {
    el.text().collect::<String>().trim().to_string()
}

pub fn attr(el: ElementRef<'_>, name: &str) -> Option<String> {
    el.value().attr(name).map(|s| s.to_string())
}

pub fn find<'a>(el: ElementRef<'a>, s: &Selector) -> Option<ElementRef<'a>> {
    el.select(s).next()
}

pub fn find_txt(el: ElementRef<'_>, s: &Selector) -> String {
    find(el, s).map(txt).unwrap_or_default()
}

pub fn find_attr(el: ElementRef<'_>, s: &Selector, name: &str) -> Option<String> {
    find(el, s).and_then(|e| attr(e, name))
}

/// cheerio `load(html).text()`，用于把 HTML 片段转成纯文本（coolapk 的多行 message）。
pub fn strip_tags(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    doc.root_element().text().collect::<String>().trim().to_string()
}

pub fn parse(html: &str) -> Html {
    Html::parse_document(html)
}

/// 取元素内第 n 个匹配（对应 cheerio 的 `.eq(n)` / `.slice(1)`）。
#[allow(dead_code)]
pub fn nth<'a>(el: ElementRef<'a>, s: &Selector, n: usize) -> Option<ElementRef<'a>> {
    el.select(s).nth(n)
}
