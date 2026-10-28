// Small helpers for Gentoo atoms and versions (PMS rules, simplified).

// Is `v` a valid PMS version? e.g. "1.2.3", "1.0b", "2.0_rc1", "3.3.2-r1"
// Grammar: \d+(\.\d+)*[a-z]?((_alpha|_beta|_pre|_rc|_p)\d*)*(-r\d+)?
pub fn is_valid_version(v: &str) -> bool
{
    let mut s = v;

    // optional "-rN" revision
    if let Some(idx) = s.rfind("-r")
    {
        let rev = &s[idx + 2..];
        if !rev.is_empty() && rev.chars().all(|c| c.is_ascii_digit())
        {
            s = &s[..idx];
        }
    }

    // optional "_suffixN" parts
    let mut parts = s.split('_');
    let mut base = parts.next().unwrap_or("");
    for suffix in parts
    {
        let digits_at = suffix.find(|c: char| c.is_ascii_digit()).unwrap_or(suffix.len());
        let (word, num) = suffix.split_at(digits_at);
        let known = matches!(word, "alpha" | "beta" | "pre" | "rc" | "p");
        if !known || !num.chars().all(|c| c.is_ascii_digit())
        {
            return false;
        }
    }

    // optional single trailing letter, e.g. "1.0b"
    if base.ends_with(|c: char| c.is_ascii_lowercase())
    {
        base = &base[..base.len() - 1];
    }

    if base.is_empty()
    {
        return false;
    }

    return base.split('.').all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()));
}

// Split "lua-lpeg-1.1.0" into ("lua-lpeg", "1.1.0").
pub fn split_pv(pv: &str) -> Option<(String, String)>
{
    for (idx, ch) in pv.char_indices()
    {
        if ch == '-' && is_valid_version(&pv[idx + 1..])
        {
            return Some((pv[..idx].to_string(), pv[idx + 1..].to_string()));
        }
    }

    return None;
}

// Reduce any dependency/config atom to plain "cat/pkg".
//   ">=dev-libs/libuv-1.48.0-r1:0/1=[static-libs?]" -> "dev-libs/libuv"
//   "dev-lang/python:3.12::gentoo"                  -> "dev-lang/python"
pub fn strip_to_cp(token: &str) -> Option<String>
{
    let mut s = token.trim();

    let is_op = |c: char| matches!(c, '<' | '>' | '=' | '~');
    let had_operator = s.starts_with(is_op);
    s = s.trim_start_matches(is_op);

    if let Some(i) = s.find('[')
    {
        s = &s[..i];
    }
    if let Some(i) = s.find(':') // also catches "::repo"
    {
        s = &s[..i];
    }
    s = s.trim_end_matches('*');

    let (category, pn) = s.split_once('/')?;
    if category.is_empty() || pn.is_empty()
    {
        return None;
    }

    if had_operator
    {
        let (name, _version) = split_pv(pn)?;
        return Some(format!("{}/{}", category, name));
    }

    return Some(format!("{}/{}", category, pn));
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn versions()
    {
        assert!(is_valid_version("1.2.3"));
        assert!(is_valid_version("3.3.2-r1"));
        assert!(is_valid_version("2.0_rc1"));
        assert!(is_valid_version("1.0b_p20240101"));
        assert!(!is_valid_version("lpeg-1.1.0"));
        assert!(!is_valid_version("misc-1.0"));
    }

    #[test]
    fn pv_split()
    {
        assert_eq!(split_pv("lua-lpeg-1.1.0"), Some(("lua-lpeg".into(), "1.1.0".into())));
        assert_eq!(split_pv("font-misc-misc-1.1.3-r1"), Some(("font-misc-misc".into(), "1.1.3-r1".into())));
        assert_eq!(split_pv("neovim"), None);
    }

    #[test]
    fn cp_strip()
    {
        assert_eq!(strip_to_cp(">=dev-libs/libuv-1.48.0-r1:0/1=[static-libs?]"), Some("dev-libs/libuv".into()));
        assert_eq!(strip_to_cp("dev-lang/python:3.12::gentoo"), Some("dev-lang/python".into()));
        assert_eq!(strip_to_cp("=app-misc/foo-1.2*"), Some("app-misc/foo".into()));
        assert_eq!(strip_to_cp("sys-libs/zlib"), Some("sys-libs/zlib".into()));
        assert_eq!(strip_to_cp("||"), None);
    }
}
