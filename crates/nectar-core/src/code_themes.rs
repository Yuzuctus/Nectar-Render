//! Thèmes de coloration du code, sur le modèle des éditeurs (VS Code…).
//!
//! Chaque thème donne les couleurs du bloc (fond, texte, gouttière des
//! numéros, bandeau) et celles des jetons. Le fichier `.tmTheme` que Typst
//! utilise pour colorer est généré à partir de ces données.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeTheme {
    pub id: &'static str,
    pub label: &'static str,
    pub dark: bool,
    pub background: &'static str,
    pub foreground: &'static str,
    /// Numéros de ligne.
    pub gutter: &'static str,
    /// Bandeau (onglet, barre de fenêtre) et bordure.
    pub header: &'static str,
    pub border: &'static str,
    pub tokens: Tokens,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tokens {
    pub comment: &'static str,
    pub keyword: &'static str,
    /// `if`, `return`, `import`… (souvent comme `keyword`).
    pub control: &'static str,
    pub string: &'static str,
    pub number: &'static str,
    pub constant: &'static str,
    pub function: &'static str,
    pub typ: &'static str,
    pub variable: &'static str,
    pub parameter: &'static str,
    pub operator: &'static str,
    pub tag: &'static str,
    pub attribute: &'static str,
    pub escape: &'static str,
    pub italic_comments: bool,
}

pub const THEMES: &[CodeTheme] = &[
    CodeTheme {
        id: "vscode-dark",
        label: "VS Code Dark+",
        dark: true,
        background: "#1e1e1e",
        foreground: "#d4d4d4",
        gutter: "#858585",
        header: "#252526",
        border: "#3c3c3c",
        tokens: Tokens {
            comment: "#6a9955",
            keyword: "#569cd6",
            control: "#c586c0",
            string: "#ce9178",
            number: "#b5cea8",
            constant: "#4fc1ff",
            function: "#dcdcaa",
            typ: "#4ec9b0",
            variable: "#9cdcfe",
            parameter: "#9cdcfe",
            operator: "#d4d4d4",
            tag: "#569cd6",
            attribute: "#9cdcfe",
            escape: "#d7ba7d",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "vscode-light",
        label: "VS Code Light+",
        dark: false,
        background: "#ffffff",
        foreground: "#000000",
        gutter: "#237893",
        header: "#f3f3f3",
        border: "#e5e5e5",
        tokens: Tokens {
            comment: "#008000",
            keyword: "#0000ff",
            control: "#af00db",
            string: "#a31515",
            number: "#098658",
            constant: "#0070c1",
            function: "#795e26",
            typ: "#267f99",
            variable: "#001080",
            parameter: "#001080",
            operator: "#000000",
            tag: "#800000",
            attribute: "#e50000",
            escape: "#ee0000",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "github-light",
        label: "GitHub clair",
        dark: false,
        background: "#f6f8fa",
        foreground: "#1f2328",
        gutter: "#8c959f",
        header: "#eaeef2",
        border: "#d0d7de",
        tokens: Tokens {
            comment: "#6e7781",
            keyword: "#cf222e",
            control: "#cf222e",
            string: "#0a3069",
            number: "#0550ae",
            constant: "#0550ae",
            function: "#8250df",
            typ: "#953800",
            variable: "#1f2328",
            parameter: "#953800",
            operator: "#cf222e",
            tag: "#116329",
            attribute: "#0550ae",
            escape: "#0a3069",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "github-dark",
        label: "GitHub sombre",
        dark: true,
        background: "#161b22",
        foreground: "#e6edf3",
        gutter: "#6e7681",
        header: "#0d1117",
        border: "#30363d",
        tokens: Tokens {
            comment: "#8b949e",
            keyword: "#ff7b72",
            control: "#ff7b72",
            string: "#a5d6ff",
            number: "#79c0ff",
            constant: "#79c0ff",
            function: "#d2a8ff",
            typ: "#ffa657",
            variable: "#e6edf3",
            parameter: "#ffa657",
            operator: "#ff7b72",
            tag: "#7ee787",
            attribute: "#79c0ff",
            escape: "#a5d6ff",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "one-dark",
        label: "One Dark",
        dark: true,
        background: "#282c34",
        foreground: "#abb2bf",
        gutter: "#636d83",
        header: "#21252b",
        border: "#181a1f",
        tokens: Tokens {
            comment: "#7f848e",
            keyword: "#c678dd",
            control: "#c678dd",
            string: "#98c379",
            number: "#d19a66",
            constant: "#d19a66",
            function: "#61afef",
            typ: "#e5c07b",
            variable: "#e06c75",
            parameter: "#e06c75",
            operator: "#56b6c2",
            tag: "#e06c75",
            attribute: "#d19a66",
            escape: "#56b6c2",
            italic_comments: true,
        },
    },
    CodeTheme {
        id: "monokai",
        label: "Monokai",
        dark: true,
        background: "#272822",
        foreground: "#f8f8f2",
        gutter: "#90908a",
        header: "#1e1f1c",
        border: "#414339",
        tokens: Tokens {
            comment: "#88846f",
            keyword: "#f92672",
            control: "#f92672",
            string: "#e6db74",
            number: "#ae81ff",
            constant: "#ae81ff",
            function: "#a6e22e",
            typ: "#66d9ef",
            variable: "#f8f8f2",
            parameter: "#fd971f",
            operator: "#f92672",
            tag: "#f92672",
            attribute: "#a6e22e",
            escape: "#ae81ff",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "dracula",
        label: "Dracula",
        dark: true,
        background: "#282a36",
        foreground: "#f8f8f2",
        gutter: "#6272a4",
        header: "#21222c",
        border: "#191a21",
        tokens: Tokens {
            comment: "#6272a4",
            keyword: "#ff79c6",
            control: "#ff79c6",
            string: "#f1fa8c",
            number: "#bd93f9",
            constant: "#bd93f9",
            function: "#50fa7b",
            typ: "#8be9fd",
            variable: "#f8f8f2",
            parameter: "#ffb86c",
            operator: "#ff79c6",
            tag: "#ff79c6",
            attribute: "#50fa7b",
            escape: "#ff79c6",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "solarized-light",
        label: "Solarized clair",
        dark: false,
        background: "#fdf6e3",
        foreground: "#657b83",
        gutter: "#93a1a1",
        header: "#eee8d5",
        border: "#e6dfc8",
        tokens: Tokens {
            comment: "#93a1a1",
            keyword: "#859900",
            control: "#859900",
            string: "#2aa198",
            number: "#d33682",
            constant: "#cb4b16",
            function: "#268bd2",
            typ: "#b58900",
            variable: "#268bd2",
            parameter: "#268bd2",
            operator: "#859900",
            tag: "#268bd2",
            attribute: "#93a1a1",
            escape: "#dc322f",
            italic_comments: true,
        },
    },
    CodeTheme {
        id: "nord",
        label: "Nord",
        dark: true,
        background: "#2e3440",
        foreground: "#d8dee9",
        gutter: "#616e88",
        header: "#3b4252",
        border: "#3b4252",
        tokens: Tokens {
            comment: "#616e88",
            keyword: "#81a1c1",
            control: "#81a1c1",
            string: "#a3be8c",
            number: "#b48ead",
            constant: "#81a1c1",
            function: "#88c0d0",
            typ: "#8fbcbb",
            variable: "#d8dee9",
            parameter: "#d8dee9",
            operator: "#81a1c1",
            tag: "#81a1c1",
            attribute: "#8fbcbb",
            escape: "#ebcb8b",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "xcode",
        label: "Xcode clair",
        dark: false,
        background: "#ffffff",
        foreground: "#262626",
        gutter: "#a6a6a6",
        header: "#f5f5f5",
        border: "#e5e5e5",
        tokens: Tokens {
            comment: "#5d6c79",
            keyword: "#9b2393",
            control: "#9b2393",
            string: "#c41a16",
            number: "#1c00cf",
            constant: "#1c00cf",
            function: "#326d74",
            typ: "#0b4f79",
            variable: "#262626",
            parameter: "#262626",
            operator: "#262626",
            tag: "#9b2393",
            attribute: "#836c28",
            escape: "#c41a16",
            italic_comments: false,
        },
    },
    CodeTheme {
        id: "agrume",
        label: "Agrume",
        dark: false,
        background: "#f3f6ea",
        foreground: "#141c17",
        gutter: "#7c8274",
        header: "#e0e5d2",
        border: "#d2d8c6",
        tokens: Tokens {
            comment: "#5f6761",
            keyword: "#0e6b4c",
            control: "#0e6b4c",
            string: "#9e2452",
            number: "#6b5a00",
            constant: "#6b5a00",
            function: "#156b85",
            typ: "#156b85",
            variable: "#141c17",
            parameter: "#4b554f",
            operator: "#141c17",
            tag: "#0e6b4c",
            attribute: "#4b554f",
            escape: "#9e2452",
            italic_comments: false,
        },
    },
];

/// Noms de thèmes Pygments de la v1 → thème équivalent.
const ALIASES: &[(&str, &str)] = &[
    ("default", "github-light"),
    ("friendly", "github-light"),
    ("native", "vscode-dark"),
    ("vim", "one-dark"),
    ("dark-plus", "vscode-dark"),
    ("light-plus", "vscode-light"),
];

pub fn get(id: &str) -> Option<&'static CodeTheme> {
    let id = ALIASES.iter().find(|(alias, _)| alias.eq_ignore_ascii_case(id)).map(|(_, to)| *to).unwrap_or(id);
    THEMES.iter().find(|t| t.id.eq_ignore_ascii_case(id))
}

impl CodeTheme {
    /// Le fichier `.tmTheme` (plist XML) lu par Typst.
    pub fn tm_theme(&self) -> String {
        let t = &self.tokens;
        let rules: &[(&str, &str, &str)] = &[
            ("Commentaires", "comment, punctuation.definition.comment", t.comment),
            ("Mots-clés", "keyword, storage.type, storage.modifier, keyword.other", t.keyword),
            ("Contrôle", "keyword.control, keyword.import, keyword.other.import", t.control),
            ("Opérateurs", "keyword.operator", t.operator),
            ("Chaînes", "string, punctuation.definition.string", t.string),
            ("Échappements", "constant.character.escape, string.regexp", t.escape),
            ("Nombres", "constant.numeric", t.number),
            (
                "Constantes",
                "constant.language, constant.character, support.constant, variable.other.constant",
                t.constant,
            ),
            (
                "Fonctions",
                "entity.name.function, support.function, meta.function-call.generic, variable.function",
                t.function,
            ),
            (
                "Types",
                "entity.name.type, entity.name.class, support.type, support.class, storage.type.primitive, entity.name.struct, entity.name.enum, entity.name.trait",
                t.typ,
            ),
            ("Variables", "variable, variable.other", t.variable),
            ("Paramètres", "variable.parameter", t.parameter),
            ("Balises", "entity.name.tag, punctuation.definition.tag", t.tag),
            (
                "Attributs",
                "entity.other.attribute-name, support.type.property-name, meta.object-literal.key",
                t.attribute,
            ),
            ("Titres", "markup.heading, entity.name.section", t.keyword),
            ("Liens", "markup.underline.link", t.string),
        ];
        let mut xml = String::from(concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\"><dict>\n",
        ));
        xml.push_str(&format!("<key>name</key><string>{}</string>\n<key>settings</key><array>\n", self.label));
        xml.push_str(&format!(
            "<dict><key>settings</key><dict><key>foreground</key><string>{}</string><key>background</key><string>{}</string></dict></dict>\n",
            self.foreground, self.background
        ));
        for (name, scope, color) in rules {
            let style = if *name == "Commentaires" && t.italic_comments { "italic" } else { "" };
            xml.push_str(&format!(
                "<dict><key>name</key><string>{name}</string><key>scope</key><string>{scope}</string><key>settings</key><dict><key>foreground</key><string>{color}</string><key>fontStyle</key><string>{style}</string></dict></dict>\n"
            ));
        }
        xml.push_str("</array></dict></plist>\n");
        xml
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ids_are_unique_and_aliases_resolve() {
        let mut ids: Vec<_> = super::THEMES.iter().map(|t| t.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), super::THEMES.len());
        assert_eq!(super::get("native").unwrap().id, "vscode-dark");
        assert!(super::get("inconnu").is_none());
        assert!(super::get("dracula").unwrap().tm_theme().contains("#ff79c6"));
    }
}
