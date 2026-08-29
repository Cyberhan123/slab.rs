/// ANSI escape-sequence parser: turns terminal output (command live streams,
/// finalized shell output) into styled text spans.
///
/// Ports the rendering semantics of the desktop `ansi-to-react` vendored
/// component at the subset mobile needs: SGR styles (bold/faint/italic/
/// underline, fg/bg incl. 256-color and RGB), `\r` carriage-return overwrite,
/// `\n` line breaks; every other C0/C1 control and non-SGR CSI sequence
/// (cursor movement, screen erase, mode switches) is stripped. Linkify is
/// intentionally out of scope.
library;

import 'package:flutter/material.dart';

/// One run of text sharing a style.
class AnsiSpan {
  const AnsiSpan({required this.text, required this.style});
  final String text;
  final AnsiStyle style;
}

class AnsiStyle {
  const AnsiStyle({
    this.bold = false,
    this.faint = false,
    this.italic = false,
    this.underline = false,
    this.foreground,
    this.background,
  });

  final bool bold;
  final bool faint;
  final bool italic;
  final bool underline;
  final Color? foreground;
  final Color? background;

  AnsiStyle copyWith({
    bool? bold,
    bool? faint,
    bool? italic,
    bool? underline,
    bool clearForeground = false,
    Color? foreground,
    bool clearBackground = false,
    Color? background,
  }) =>
      AnsiStyle(
        bold: bold ?? this.bold,
        faint: faint ?? this.faint,
        italic: italic ?? this.italic,
        underline: underline ?? this.underline,
        foreground: clearForeground ? null : (foreground ?? this.foreground),
        background: clearBackground ? null : (background ?? this.background),
      );
}

/// Classic xterm palette for SGR 30-37 / 90-97 (fg) and 40-47 / 100-107 (bg).
/// Build a Color from a 0xRRGGBB value (terminal palettes are DATA 
/// -- the xterm palette -- not design tokens, hence a constructor 
/// instead of theme lookups).
Color _rgb(int hex) =>
    Color.fromARGB(0xFF, (hex >> 16) & 0xFF, (hex >> 8) & 0xFF, hex & 0xFF);

final _basicPalette = <int, Color> {
  0: _rgb(0x000000), 1: _rgb(0xCD3131), 2: _rgb(0x0DBC79), 3: _rgb(0xE5E510),
  4: _rgb(0x2472C8), 5: _rgb(0xBC3FBC), 6: _rgb(0x11A8CD), 7: _rgb(0xE5E5E5),
  8: _rgb(0x666666), 9: _rgb(0xF14C4C), 10: _rgb(0x23D18B), 11: _rgb(0xF5F543),
  12: _rgb(0x3B8EEA), 13: _rgb(0xD670D6), 14: _rgb(0x29B8DB), 15: _rgb(0xFFFFFF),
};

Color _color256(int index) {
  if (index < 16) return _basicPalette[index] ?? _rgb(0x000000);
  if (index < 232) {
    final n = index - 16;
    const levels = [0, 95, 135, 175, 215, 255];
    final r = levels[(n ~/ 36) % 6];
    final g = levels[(n ~/ 6) % 6];
    final b = levels[n % 6];
    return _rgb((r << 16) | (g << 8) | b);
  }
  final gray = 8 + (index - 232) * 10;
  return _rgb((gray << 16) | (gray << 8) | gray);
}

final _csiPattern = RegExp('\x1b\\[([0-9;?]*)([A-Za-z])');

/// Parse [input] into spans. Unknown sequences are dropped, not fatal.
List<AnsiSpan> parseAnsi(String input) {
  final spans = <AnsiSpan>[];
  var style = const AnsiStyle();
  var buffer = StringBuffer();

  // Carriage-return overwrite: characters after \r replace the current line.
  List<String> lines = input.split('\n');
  for (var lineIndex = 0; lineIndex < lines.length; lineIndex++) {
    if (lineIndex > 0) {
      spans.add(AnsiSpan(text: '\n', style: style));
    }
    final line = _applyCarriageReturns(lines[lineIndex]);
    var rest = line;
    while (true) {
      final match = _csiPattern.firstMatch(rest);
      if (match == null) {
        if (rest.isNotEmpty) buffer.write(rest);
        break;
      }
      if (match.start > 0) buffer.write(rest.substring(0, match.start));
      if (match.group(2) == 'm') {
        _flush(spans, buffer, style);
        style = _applySgr(match.group(1) ?? '', style);
      }
      // Non-SGR CSI sequences (cursor movement etc.) are dropped.
      rest = rest.substring(match.end);
    }
  }
  _flush(spans, buffer, style);
  return spans;
}

void _flush(List<AnsiSpan> spans, StringBuffer buffer, AnsiStyle style) {
  if (buffer.isNotEmpty) {
    spans.add(AnsiSpan(text: buffer.toString(), style: style));
    buffer.clear();
  }
}

/// Collapse `\r` overwrites inside a single line: text after the last `\r`
/// (before any `\n`) replaces the line from column 0.
String _applyCarriageReturns(String line) {
  final parts = line.split('\r');
  if (parts.length == 1) return line;
  // Classic terminal semantics: each \r rewinds to column 0, subsequent
  // characters overwrite left-to-right.
  var result = '';
  for (final segment in parts) {
    if (segment.length >= result.length) {
      result = segment;
    } else {
      result = segment + result.substring(segment.length);
    }
  }
  return result;
}

AnsiStyle _applySgr(String params, AnsiStyle current) {
  if (params.isEmpty || params == '0') return const AnsiStyle();
  final codes = params.split(';').map((code) => int.tryParse(code) ?? 0).toList(growable: false);
  var style = current;
  var i = 0;
  while (i < codes.length) {
    final code = codes[i];
    switch (code) {
      case 0:
        style = const AnsiStyle();
      case 1:
        style = style.copyWith(bold: true);
      case 2:
        style = style.copyWith(faint: true);
      case 3:
        style = style.copyWith(italic: true);
      case 4:
        style = style.copyWith(underline: true);
      case 22:
        style = style.copyWith(bold: false, faint: false);
      case 23:
        style = style.copyWith(italic: false);
      case 24:
        style = style.copyWith(underline: false);
      case 39:
        style = style.copyWith(clearForeground: true);
      case 49:
        style = style.copyWith(clearBackground: true);
      default:
        if (code >= 30 && code <= 37) {
          style = style.copyWith(foreground: _basicPalette[code - 30]);
        } else if (code >= 90 && code <= 97) {
          style = style.copyWith(foreground: _basicPalette[code - 90 + 8]);
        } else if (code >= 40 && code <= 47) {
          style = style.copyWith(background: _basicPalette[code - 40]);
        } else if (code >= 100 && code <= 107) {
          style = style.copyWith(background: _basicPalette[code - 100 + 8]);
        } else if (code == 38 || code == 48) {
          final (color, consumed) = _extendedColor(codes, i);
          if (color != null) {
            style = code == 38 ? style.copyWith(foreground: color) : style.copyWith(background: color);
          }
          i += consumed;
        }
    }
    i += 1;
  }
  return style;
}

/// Parse `38;5;n` / `38;2;r;g;b` starting at [i] (the 38/48 itself).
(Color?, int) _extendedColor(List<int> codes, int i) {
  if (i + 1 >= codes.length) return (null, 1);
  switch (codes[i + 1]) {
    case 5:
      if (i + 2 < codes.length) return (_color256(codes[i + 2]), 2);
      return (null, 2);
    case 2:
      if (i + 4 < codes.length) {
        return (_rgb((codes[i + 2] << 16) | (codes[i + 3] << 8) | codes[i + 4]), 4);
      }
      return (null, 4);
    default:
      return (null, 1);
  }
}
