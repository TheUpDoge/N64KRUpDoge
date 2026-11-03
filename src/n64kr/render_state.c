#include <libultraship.h>
#include <defines.h>

#include "main.h"
#include "n64kr/render_state.h"

void n64kr_restore_track_state(void) {
    gDPPipeSync(gDisplayListHead++);
    gSPSetGeometryMode(gDisplayListHead++, G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH);
    gDPSetTexturePersp(gDisplayListHead++, G_TP_PERSP);
    gDPSetTextureFilter(gDisplayListHead++, G_TF_BILERP);
    gDPSetTextureConvert(gDisplayListHead++, G_TC_FILT);
}
