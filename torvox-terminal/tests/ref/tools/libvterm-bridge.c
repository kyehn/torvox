/* libvterm-bridge.c — C reference bridge for Rust FFI.
 * Wraps libvterm state into a simple snapshot for cross-reference.
 */
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include "vterm.h"

typedef struct {
    int rows;
    int cols;
    char *text;
    int cursor_row;
    int cursor_col;
} VTermSnapshot;

VTermSnapshot* vterm_ref_snapshot(const uint8_t *seq, size_t len, int rows, int cols) {
    VTerm *vt = vterm_new(rows, cols);
    if (!vt) return NULL;
    VTermScreen *screen = vterm_obtain_screen(vt);
    VTermState *state = vterm_obtain_state(vt);
    vterm_state_reset(state, 1);
    vterm_input_write(vt, (const char*)seq, len);

    VTermSnapshot *snap = malloc(sizeof(VTermSnapshot));
    snap->rows = rows;
    snap->cols = cols;
    snap->text = malloc((size_t)(rows * cols));
    memset(snap->text, ' ', (size_t)(rows * cols));

    for (int r = 0; r < rows; r++) {
        for (int c = 0; c < cols; c++) {
            VTermPos pos = { .row = r, .col = c };
            VTermScreenCell cell;
            if (vterm_screen_get_cell(screen, pos, &cell)) {
                snap->text[r * cols + c] = (char)(cell.chars[0] & 0xFF);
            }
        }
    }

    VTermPos cursor;
    vterm_state_get_cursorpos(state, &cursor);
    snap->cursor_row = cursor.row;
    snap->cursor_col = cursor.col;

    vterm_free(vt);
    return snap;
}

void vterm_free_snapshot(VTermSnapshot *snap) {
    if (snap) {
        free(snap->text);
        free(snap);
    }
}
