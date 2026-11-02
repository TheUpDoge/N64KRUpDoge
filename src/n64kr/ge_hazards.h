#ifndef N64KR_GE_HAZARDS_H
#define N64KR_GE_HAZARDS_H

// N64KR: GoldenEye hazards for the Dam tracks (recon/008-goldeneye-dam):
// exploding oil drums on the road and Dam guards who raise their KF7s and
// fire bursts at passing racers. Models and sprites come from your ROM
// (tools/n64kr-rs ge-hazards -> mod n64kr-ge-assets, ge/...), sounds too
// (ge-audio -> sound/n64kr/ge/). Tracks place them with scene.json actors
// "n64kr:ge_barrel" and "n64kr:ge_guard" (Skin: guard head, Rotation: facing).

#ifdef __cplusplus
#include "engine/registry/RegisterContent.h"

// Appends N64KR's actor types to the registry (RegisterActors, N64KR HOOK;
// after the stock ones so their ids don't move).
void n64kr_register_actors(Registry<ActorInfo, const SpawnParams&>& r);
#endif

#endif // N64KR_GE_HAZARDS_H
