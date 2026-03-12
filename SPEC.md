# Rip

A markup language for receipts. Each line stands alone.

## Text

Just type. It's text.

```
Hello world
```

### Styles

Wrap with a single character. No nesting.

```
*bold*
_underline_
`italic`
~strikethrough~
```

### Sizes

Markers on both sides of the line. More symbols = bigger.

```
plain text                        (text — default)
++ medium body ++                 (text-m)
+++ large body +++                (text-l)
## small header ##                (title)
### medium header ###             (title-m)
#### large header ####            (title-l)
```

Sizes wrap entire lines, including columns:

```
++ | *TOTAL* |> *$19.74* | ++
#### BURGER BARN ####
```

Closing marker is optional — `## HELLO` works fine.

## Columns

Pipes make columns. Must start and end with `|`.

```
| left | right |
| a | b | c |
```

### Alignment

`>` pushes right, `<` pushes left. Both = center.

```
| plain |                         left (default)
|> pushed right |                 right
|> centered <|                    center
```

Multi-column:

```
| Item |> $8.99 |                 left + right
| Qty |> Item <| Price |          left + center + right
```

2 columns auto-align left + right. 3 columns auto-align left + center + right. Override any cell with `>` or `<`.

### Column widths

By default, columns auto-size based on content. Prefix a cell with a number to set its width as a percentage:

```
|80 Description |20> Price |      80/20 split
|60 Item |20> Qty |20> Price |    60/20/20 split
|80 Wide column | Auto rest |     one explicit, rest auto
```

If explicit percentages exceed 100%, they are ignored and columns fall back to equal widths.

### Dividers in columns

```
| Item | --- |> $8.99 |
| *Total* | === |
```

## Dividers

Three or more of the same character:

```
---          thin line
===          thick line
...          dotted line
```

Longer is fine: `----------`

## Comments

```
// ignored
```

## Blank lines

Empty lines produce a line feed in the output.

## Escaping

Backslash escapes the next character:

```
\* \| \# \+ \_ \~ \` \@ \\
```

## Directives

`@name(args)` — comma-separated, no quotes.

### Printer config

```
@printer-width(80mm)              default 80mm
@printer-width(58mm)              compact thermal
@printer-width(3in)               inches work too
@printer-width(8cm)               so do cm
@printer-dpi(203)                 default 203 (standard thermal)
@printer-dpi(300)                 high-res thermal
@printer-threshold(128)           default 128 (0–255, black/white cutoff)
```

### Fonts

```
@style(text, https://example.com/Mono.ttf, 12)
@style(title, https://example.com/Bold.ttf, 24)
@style(text, default, 12)         reset to built-in font
```

Levels: `text`, `text-m`, `text-l`, `title`, `title-m`, `title-l`.

#### Bold font variants

Append `-bold` to any level name to define its bold font:

```
@style(text, Inter-Regular.ttf, 12)
@style(text-bold, Inter-Bold.ttf, 12)
@style(title-bold, Inter-ExtraBold.ttf, 24)
```

When `*bold*` text is rendered, the bold font is used if defined. If no bold font is available, a faux bold (pixel offset) is used as fallback.

Setting a regular `@style` clears any previously set bold variant for that level.

### Images

Two directives for images, with different rendering modes:

```
@image(photo.jpg)                 dithered (Floyd-Steinberg) — best for photos
@image(photo.jpg, 200)            width in pt
@image(photo.jpg, 200, 100)       width + height in pt
|> @image(photo.jpg) <|           centered
```

```
@logo(logo.png)                   threshold — best for logos, icons, line art
@logo(logo.png, 125)              width in pt
@logo(logo.png, 125, 80)          width + height in pt
|> @logo(logo.png) <|             centered
```

**`@image` vs `@logo`**: `@image` applies Floyd-Steinberg error-diffusion dithering, which produces smooth gradients for photographs. `@logo` uses simple black/white threshold, which keeps crisp edges for logos, icons, and line art. The threshold is controlled by `@printer-threshold()`.

#### Supported formats

PNG, JPEG, GIF, BMP, WebP, and SVG. SVG is rendered at the target resolution for sharp output at any size.

#### Scaling behavior

- No dimensions: scales to full paper width
- Width only: scales to width, no upscaling past natural size
- Width + height: fits within both bounds, aspect ratio preserved

#### Resource paths

Image and font paths can be:

- **Relative paths** — resolved from the configured resource directory (e.g., `@image(logo.png)`, `@image(resources/receipt.png)`)
- **HTTPS URLs** — fetched automatically (e.g., `@image(https://example.com/logo.png)`)

### QR codes

```
@qr(https://example.com)
@qr(https://example.com, 200)     size in pt
|> @qr(https://example.com) <|    centered
```

### Barcodes

```
@barcode(CODE128, ABC-123)
@barcode(EAN13, 4006381333931)
@barcode(EAN8, 96385074)
@barcode(CODE39, ABC123)
@barcode(CODABAR, A123456B)
```

### Printer commands

```
@cut()                            full cut
@cut(partial)                     partial cut
@feed(3)                          feed 3 blank lines
@feed(.5)                         half a line feed
@feed(1/2)                        half a line feed (fraction)
@feed(2mm)                        2mm vertical space
@feed(.5in)                       half inch vertical space
@drawer()                         open cash drawer
```

## Example

```
@printer-width(80mm)
@style(title, default, 24)

|> @logo(logo.png, 150) <|
#### BURGER BARN ####
|> 742 Evergreen Terrace <|
|> (555) 867-5309 <|

===

| Order #1042 |> 02/25/2026 |
| Cashier: Homer S. |> 12:34 PM |

---

| Classic Burger |> $8.99 |
| Cheese Fries |> $4.50 |
|   `add bacon` |> $1.50 |
| Lg Lemonade |> $3.25 |

...

| Subtotal |> $18.24 |
| Tax (8.25%) |> $1.50 |
===
++ | *TOTAL* |> *$19.74* | ++

---

| Visa x4821 |> $19.74 |

===

|> @qr(https://burgerbarn.com/receipt/1042) <|
|> Thank you for dining with us! <|

@feed(2)
@cut()
@drawer()
```
