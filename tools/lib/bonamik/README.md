# Bonamik reference evaluator

Node transcription of the Bonamik (BonamikRt / KBDRt) secondary-motion solver as it runs for Sora, written by the
research pass `build/sora_tmp/native_H_bonamik2.md` (specification: `docs/sora-animation-spec.md` section 14).

- `bnm_eval.js` — the evaluator: `create({ refPose, parents, boneNames, ... })`, `sim.frame(dt, animatedComponentSpacePose)`.
  Poses are UE component space, cm, `{q:[x,y,z,w], t:[x,y,z], s:[x,y,z]}` per bone. Data: `audit/sora_port/bonamik.json`.
- `m.js` (float32 helpers), `core.js`, `cone.js`, `chain.js`, `coll.js`, `kdi.js`, `reset.js`, `wind.js`, `hfps.js` — the transcribed functions.
- `anim.js` — poses from the pack (`build/sora_pack`) for the stand-alone validation.
- `validate.js` — `node tools/lib/bonamik/validate.js [evaluations]`: rest stability, length invariants, determinism, frame-rate independence of the 30 Hz evaluation states.

`tools/sora_pack.js` runs a subset of these validations on every build and `--check`.
