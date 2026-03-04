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
```

### Fonts

```
@style(text, https://example.com/Mono.ttf, 12)
@style(title, https://example.com/Bold.ttf, 24)
@style(text, default, 12)         reset to built-in font
```

Levels: `text`, `text-m`, `text-l`, `title`, `title-m`, `title-l`.

### Images

```
@image(logo.png)
@image(logo.png, 200)             width in pt
@image(logo.png, 200, 100)        width + height in pt
|> @image(logo.png) <|            centered
```

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
@drawer()                         open cash drawer
```

## Example

```
@printer-width(80mm)
@style(title, default, 24)

|> @image(logo.png, 150) <|
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
