//! Pools used to synthesise plausible author names.

use rand::RngExt;

use crate::model::{NameRule, NameStyle};

/// Common Chinese family names.
pub const SURNAMES: &[&str] = &[
    "王", "李", "张", "刘", "陈", "杨", "黄", "赵", "吴", "周", "徐", "孙", "马", "朱", "胡", "郭",
    "何", "高", "林", "罗", "郑", "梁", "谢", "宋", "唐", "许", "韩", "冯", "邓", "曹", "彭", "曾",
    "肖", "田", "董", "袁", "潘", "于", "蒋", "蔡", "余", "杜", "叶", "程", "苏", "魏", "吕", "丁",
    "任", "沈", "姚", "卢", "姜", "崔", "钟", "谭", "陆", "汪", "范", "金", "石", "廖", "贾", "夏",
    "韦", "傅", "方", "白", "邹", "孟", "熊", "秦", "邱", "江", "尹", "薛", "闫", "段", "雷", "侯",
    "龙", "史", "陶", "黎", "贺", "顾", "毛", "郝", "龚", "邵", "万", "钱", "严", "覃", "武", "戴",
    "莫", "孔", "向", "汤", "简", "施", "温", "康", "常", "牛", "樊", "纪", "舒", "屈", "项", "祝",
    "阮", "蓝", "闵", "席", "季", "麻", "强", "路", "娄", "危", "童", "颜", "梅", "盛", "倪", "芦",
    "习", "岑", "裴", "霍", "虞", "曲", "岳",
];

/// Characters frequently used in Chinese given names.
pub const GIVEN_CHARS: &[&str] = &[
    "伟", "芳", "娜", "敏", "静", "丽", "强", "磊", "军", "洋", "勇", "艳", "杰", "娟", "涛", "明",
    "超", "秀", "霞", "平", "刚", "桂", "英", "华", "玉", "萍", "红", "玲", "芬", "燕", "彬", "宇",
    "浩", "轩", "然", "睿", "泽", "楠", "欣", "怡", "婷", "悦", "涵", "晨", "曦", "阳", "雪", "琳",
    "佳", "璐", "珊", "宁", "健", "峰", "亮", "鹏", "辉", "建", "国", "志", "文", "武", "俊", "凯",
    "帅", "飞", "云", "海", "波", "江", "山", "石", "森", "木", "青", "春", "夏", "秋", "冬", "天",
    "宇", "思", "远", "子", "梓", "梓", "若", "思", "语", "书",
];

/// Common English given names.
pub const EN_FIRST: &[&str] = &[
    "James", "Emily", "Michael", "Sarah", "Daniel", "Olivia", "Ethan", "Emma", "Noah", "Sophia",
    "Liam", "Grace", "Lucas", "Chloe", "Henry", "Alice", "Jack", "Ivy", "Leo", "Nora", "Ryan",
    "Ella", "Owen", "Mia", "Adam", "Zoe", "Eric", "Lily", "Peter", "Anna", "Tom", "Kate", "Jason",
    "Ruby", "Victor", "Clara", "Alan", "Helen", "Bruce", "Diana",
];

/// Common English family names.
pub const EN_LAST: &[&str] = &[
    "Smith", "Johnson", "Brown", "Miller", "Davis", "Wilson", "Taylor", "Clark", "Lewis", "Walker",
    "Hall", "Young", "King", "Wright", "Green", "Baker", "Adams", "Nelson", "Carter", "Mitchell",
    "Turner", "Parker", "Collins", "Edwards", "Stewart", "Morris", "Murphy", "Cook", "Rogers",
    "Cooper", "Bailey", "Reed", "Kelly", "Howard", "Brooks", "Foster", "Gray", "Price", "Watson",
    "Sanders",
];

fn pick<'a>(pool: &[&'a str], rng: &mut impl RngExt) -> &'a str {
    if pool.is_empty() {
        return "";
    }
    let index = rng.random_range(0..pool.len());
    pool.get(index).copied().unwrap_or_default()
}

/// One Chinese name, mostly three characters.
pub fn chinese_name(rng: &mut impl RngExt) -> String {
    let surname = pick(SURNAMES, rng);
    let first = pick(GIVEN_CHARS, rng);
    if rng.random_bool(0.35) {
        surname.to_string()
    } else if rng.random_bool(0.7) {
        format!("{surname}{first}")
    } else {
        let second = pick(GIVEN_CHARS, rng);
        format!("{surname}{first}{second}")
    }
}

/// One English name such as `Emily Carter`.
pub fn english_name(rng: &mut impl RngExt) -> String {
    format!("{} {}", pick(EN_FIRST, rng), pick(EN_LAST, rng))
}

/// Produce a random author according to the user's rule.
pub fn random_name(rule: &NameRule, rng: &mut impl RngExt) -> String {
    if rule.use_custom && !rule.custom.is_empty() {
        let index = rng.random_range(0..rule.custom.len());
        if let Some(name) = rule.custom.get(index) {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    let style = match rule.style {
        NameStyle::Chinese => NameStyle::Chinese,
        NameStyle::English => NameStyle::English,
        NameStyle::Mixed => {
            if rng.random_bool(0.6) {
                NameStyle::Chinese
            } else {
                NameStyle::English
            }
        }
    };
    match style {
        NameStyle::English => english_name(rng),
        _ => chinese_name(rng),
    }
}

/// A short random revision counter rendered as text (`1..=40`).
pub fn random_revision(rng: &mut impl RngExt) -> String {
    rng.random_range(1..=40u32).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_pool_wins() {
        let rule = NameRule {
            style: NameStyle::English,
            use_custom: true,
            custom: vec!["林可".to_string(), "赵野".to_string()],
        };
        let mut rng = rand::rng();
        for _ in 0..20 {
            let name = random_name(&rule, &mut rng);
            assert!(rule.custom.contains(&name), "unexpected name {name}");
        }
    }

    #[test]
    fn styles_produce_non_empty_names() {
        let mut rng = rand::rng();
        for style in [NameStyle::Chinese, NameStyle::English, NameStyle::Mixed] {
            let rule = NameRule {
                style,
                use_custom: false,
                custom: Vec::new(),
            };
            for _ in 0..50 {
                assert!(!random_name(&rule, &mut rng).trim().is_empty());
            }
        }
    }
}
