/* libvterm-ref.c — standalone reference executable.
 * Usage: ./libvterm-ref <hex_sequence>
 * Output: first line of screen text
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <ctype.h>
#include "vterm.h"

static int hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return 0;
}

static uint8_t *hex_decode(const char *hex, size_t *out_len) {
    size_t len = strlen(hex) / 2;
    uint8_t *buf = malloc(len);
    for (size_t i = 0; i < len; i++) {
        buf[i] = (uint8_t)(hex_val(hex[i * 2]) * 16 + hex_val(hex[i * 2 + 1]));
    }
    *out_len = len;
    return buf;
}

int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "Usage: %s <hex_sequence>\n", argv[0]); return 1; }
    size_t len;
    uint8_t *seq = hex_decode(argv[1], &len);
    if (!seq) { fprintf(stderr, "Bad hex\n"); return 1; }

    int rows = 24, cols = 80;
    VTerm *vt = vterm_new(rows, cols);
    VTermScreen *screen = vterm_obtain_screen(vt);
    VTermState *state = vterm_obtain_state(vt);

    /* Reset terminal to initialize encodings and default state */
    vterm_state_reset(state, 1);

    /* Feed VT sequence to parser */
    vterm_input_write(vt, seq, len);

    /* Output screen as text lines */
    char line[256];
    for (int r = 0; r < rows; r++) {
        VTermRect rect = { .start_row = r, .end_row = r + 1, .start_col = 0, .end_col = cols };
        size_t n = vterm_screen_get_text(screen, line, sizeof(line) - 1, rect);
        line[n] = '\0';
        printf("%s\n", line);
    }

    VTermPos cursor;
    vterm_state_get_cursorpos(state, &cursor);
    fprintf(stderr, "CURSOR: %d %d\n", cursor.row, cursor.col);

    free(seq);
    vterm_free(vt);
    return 0;
}
