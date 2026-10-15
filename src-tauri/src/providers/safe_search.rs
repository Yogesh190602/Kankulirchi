//! Safe search. Kankulirchi only shows safe-for-work wallpapers.
//!
//! Each provider filters out adult content in its own way: Wallhaven by
//! purity, Unsplash by its strict content filter, Wikimedia Commons by
//! searching only scenery categories, and Pexels by its upload rules. This
//! module adds a check that comes first: searches for adult or suggestive
//! keywords are refused before any provider is asked. It follows Wallhaven's
//! line between "SFW" and "sketchy", so swimwear and lingerie are refused
//! too.

use crate::error::{AppError, AppResult};

/// Keywords refused wherever they appear, even inside another word
/// ("hentaiart").
const BLOCKED_FRAGMENTS: &[&str] =
    &["porn", "hentai", "nsfw", "xxx", "lingerie", "topless", "onlyfans", "playboy", "bdsm", "18+"];

/// Keywords refused only as whole words, because they occur inside
/// harmless words ("sex" in "Essex", "nude" in "denuded").
const BLOCKED_WORDS: &[&str] = &[
    "sex",
    "sexy",
    "sexual",
    "nude",
    "nudes",
    "nudity",
    "naked",
    "erotic",
    "erotica",
    "ecchi",
    "r18",
    "boob",
    "boobs",
    "tits",
    "nipple",
    "nipples",
    "genitals",
    "fetish",
    "milf",
    "bikini",
    "bikinis",
    "thong",
    "stripper",
    "striptease",
    "underwear",
    "panties",
];

const REFUSAL: &str = "Kankulirchi only shows safe-for-work wallpapers. Try a different keyword.";

/// Refuses search keywords asking for adult content.
pub fn check_keyword(keyword: &str) -> AppResult<()> {
    let keyword = keyword.to_lowercase();
    let blocked = BLOCKED_FRAGMENTS.iter().any(|fragment| keyword.contains(fragment))
        || keyword.split(|c: char| !c.is_alphanumeric()).any(|word| BLOCKED_WORDS.contains(&word));
    if blocked {
        return Err(AppError::Validation(REFUSAL.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_adult_keywords() {
        for keyword in ["nude", "Sexy girl", "NSFW anime", "hentaiart", "beach bikini", "girls 18+", "lingerie-model"] {
            assert!(check_keyword(keyword).is_err(), "{keyword} should be refused");
        }
    }

    #[test]
    fn allows_ordinary_keywords() {
        for keyword in ["mountains", "Essex coast", "denuded hills", "Sussex", "night sky", "Tokyo 2020", "sextant"] {
            assert!(check_keyword(keyword).is_ok(), "{keyword} should be allowed");
        }
    }

    #[test]
    fn refusal_explains_why() {
        let error = check_keyword("naked").unwrap_err();
        assert!(error.to_string().contains("safe-for-work"));
    }
}
