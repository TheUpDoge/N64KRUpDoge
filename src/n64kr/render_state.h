#ifndef N64KR_RENDER_STATE_H
#define N64KR_RENDER_STATE_H

// N64KR: render state fixes between MK64's draw passes.

#include <common_structs.h>

#ifdef __cplusplus
extern "C" {
#endif

// From render_course_actors (src/racing/actors.c), before the items and other
// course actors are drawn: restores the state MK64's own track lists leave.
//
// The scene setup turns on G_ZBUFFER | G_SHADE | G_CULL_BACK | G_SHADING_SMOOTH
// with perspective-correct, bilinear textures before the track
// (skybox_and_splitscreen.c, render_courses.c), and MK64's track lists keep it,
// so actor lists (bananas, shells, item boxes) never set it themselves. Custom
// tracks exported with Fast64 end every mesh with a material revert list
// (mat_revert_*) that clears G_ZBUFFER, G_SHADE and G_SHADING_SMOOTH and sets
// point filtering and no perspective correction, so items drew through hills
// (user playtest, Fossil Canyon) [CONCLUSION from the exported lists: the
// revert list's ClearGeometryMode G_ZBUFFER; banana list render mode
// G_RM_AA_ZB_TEX_EDGE needs the geometry flag to test depth].
void n64kr_restore_track_state(void);

#ifdef __cplusplus
}
#endif

#endif // N64KR_RENDER_STATE_H
