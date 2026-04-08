# Specimen: Markdown Layout

Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.

---

## Typography

Lorem ipsum dolor sit amet, consectetuer adipiscing elit **bold**, *italic*, ***bold italic***, `inline code`, and ~~strikethrough~~.
Extended text formatting (if extensions are enabled): ==highlighted text==, H~2~O (subscript), X^2^ (superscript), and keyboard inputs like <kbd>Ctrl</kbd> + <kbd>C</kbd>.

### Third-Level Heading

#### Fourth-Level Heading

##### Fifth-Level Heading

###### Sixth-Level Heading

---

## Lists

### Unordered

- Vestibulum ante ipsum primis
- In faucibus orci luctus
  - Phasellus ullamcorper ipsum rutrum nunc
  - Nunc nonummy metus
- Morbi mollis tellus ac sapien

### Ordered

1. Praesent porttitor, nulla vitae posuere
2. Egestas, aliquam ante auctor
   1. Mauris turpis nunc
   2. Blandit et
   3. Volutpat molestie
3. Porta ut, ligula

### Task List

- [x] Suspendisse eu ligula
- [x] Nulla facilisi
- [ ] Sed lectus
- [ ] Maecenas nec odio

### Definition List

Lorem
: Ipsum dolor sit amet, consectetur adipiscing elit.

Aenean 
: Commodo ligula eget dolor massa.

---

## Links

- **Inline Link:** [Vestibulum aliquet](https://example.com)
- **Reference Link:** Check out [Phasellus][python_ref] sed enim.
- **Auto-link:** <https://example.com>
- **Email Link:** <contact@example.com>

[python_ref]: https://example.com "Exemplum Website"

---

## Blockquotes

> Mauris turpis nunc, blandit et, volutpat molestie, porta ut, ligula. Fusce pharetra convallis urna.

> **Quisque rutrum:**
>
> > Aenean imperdiet. Etiam ultricies nisi vel augue. Curabitur ullamcorper `inline code` ultricies nisi.

---

## Tables

| Vestibulum | Suspendisse | Pellentesque |
|---|---|---|
| Nulla id | Donec id | Praesent adipiscing |
| Aliquam lorem | Vivamus in | Sed lectus |
| Cras dapibus | Fusce a | Curabitur in |
| Aenean ut | Etiam pretium | Ut id |
| Maecenas | Nulla sit | In in |
| Integer ante | Vestibulum | Nullam accumsan |

### Table Formatting & Alignments

| Left-Aligned | Center-Aligned | Right-Aligned |
| :--- | :---: | ---: |
| `aliquam lorem` | `vivamus in` | `sed lectus` |
| **cras dapibus** | *fusce a* | ~~curabitur~~ |

---

## Advanced Elements

### Admonitions / Callouts

> [!info] Praesent egestas
> Suspendisse nisl elit, rhoncus eget, elementum ac, condimentum eget, diam.

!!! warning "Aenean commodo"
    Donec elit libero, eleifend nec, rutrum in, mattis nec, saepius.

### Math & Equations

**Inline Math:** Vestibulum rutrum, mi nec $e^{i\pi} + 1 = 0$ elementum.

**Block Math:**

$$
f(x) = \int_{-\infty}^\infty\hat f(\xi)\,e^{2 \pi i \xi x}\,d\xi
$$

### HTML Fallbacks

<details>
  <summary>Click to expand hidden content (Interactive Details)</summary>
  <p>This content is hidden by default and uses native HTML <code>&lt;details&gt;</code> and <code>&lt;summary&gt;</code> tags. Useful for spoilers or extra info.</p>
</details>

---

## Code Blocks

### Python

```python
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Document:
    title: str
    path: Path
    page_count: int = 0

    def summary(self) -> str:
        return f"{self.title} ({self.page_count} pages)"


docs = [Document("Report", Path("report.md"), 12)]
for doc in docs:
    print(doc.summary())
```

### JavaScript

```javascript
async function convertMarkdown(filePath) {
  const content = await fs.readFile(filePath, "utf-8");
  const html = marked.parse(content);
  return { html, wordCount: content.split(/\s+/).length };
}
```

### JSON Configuration

```json
{
  "preset": "Magazine",
  "style": {
    "body_font": "Segoe UI",
    "code_theme": "default",
    "margin_mm": 25.4
  }
}
```

### SQL

```sql
SELECT p.name, COUNT(*) AS exports
FROM presets p
JOIN conversions c ON c.preset_id = p.id
GROUP BY p.name
ORDER BY exports DESC;
```

### Bash

```bash
#!/usr/bin/env bash
python -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
pytest -q
```

<!-- pagebreak -->

## Images

### Relative Path

![Service Overview](assets/service-overview.svg)

### Obsidian Embed

![[sequence.svg|Architecture Sequence Diagram]]

---

## Footnotes

Pellentesque libero tortor, tincidunt et, tincidunt eget, semper nec, quam[^1]. Maecenas ullamcorper, dui et placerat feugiat, eros pede varius nisi[^2].

[^1]: Sed aliquam ultrices mauris.

[^2]: Fusce neque. Donec vitae vitae, eleifend ac.

---

## Page Break Markers

Aenean leo ligula, porttitor eu, consequat vitae, eleifend ac, enim.

- `<!-- pagebreak -->` (HTML comment)
- `\pagebreak` (LaTeX style)
- `[[PAGEBREAK]]` (Wiki style)

\pagebreak

## Pagination Classes

### Keep With Next {.keep-with-next}

Aenean leo ligula, porttitor eu, consequat vitae, eleifend ac, enim.

- Aliquam ante
- Dictum posuere
- Orci luctus

### Keep Together {.keep-together}

In enim justo, rhoncus ut, imperdiet a, venenatis vitae, justo. Nullam dictum felis eu pede mollis pretium. Integer tincidunt. 

[[PAGEBREAK]]

## Final Notes

Donec vitae sapien ut libero venenatis faucibus. Nullam quis ante. Etiam sit amet orci eget eros faucibus tincidunt.
