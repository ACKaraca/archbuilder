# Visualization

This area specifies presentation of the model: the real-time viewport, sun and shadows, cameras,
walkthroughs, appearance, lighting, and photoreal output through RenderBuilder over the Arc Scene
contract. ArchBuilder does not contain a path tracer.

## Domain objects

| Object | Key properties | Relationships |
|---|---|---|
| Sun setting | Location, date, time or range, interval | Read by views, studies and render scenes |
| Camera | Eye, target, field of view, crop | Belongs to a 3D view |
| Walkthrough | Keyframes, path, duration, frame rate | Produces frames |
| Presentation state | Camera, sun, visual style, visibility, section | Named, recallable |
| Light source | Photometric data, color, intensity, group | Defined in light fixture families |
| Render job | Arc Scene snapshot, settings, outputs | Sent to RenderBuilder |

## Real-time display

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIS-001 | Real-time viewport | Displays models with shading, edges, shadows, ambient occlusion and transparency at interactive frame rates for large models, using GPU-driven rendering. | T1 | 2 |
| AB-VIS-002 | Sun and shadows | Positions the sun from the document's location, date and time, casts shadows in views and shows a 3D sun path diagram. | T2 | 2 |
| AB-VIS-003 | Cameras | Places perspective cameras with eye height, target, field of view and crop, and edits them by grips. | T1 | 2 |
| AB-VIS-004 | Presentation states | Saves named states of camera, sun, visual style, visibility and section, and transitions between them. | T2 | 2 |
| AB-VIS-005 | Appearance editing | Edits material appearance on the Arc Scene material model and maps textures per face with scale, rotation and offset. | T2 | 2 |
| AB-VIS-006 | Sketch-style display | Draws edges in a hand-drawn style with line jitter and extension. | T3 | 3 |

## Studies and rendering

| ID | Capability | Behavior | Tier | Phase |
|---|---|---|---|---|
| AB-VIS-007 | Solar studies | Animates shadows over a day or over several days at an interval and exports frames. | T2 | 4 |
| AB-VIS-008 | Walkthroughs | Records keyframed camera paths and exports frames from the viewport or through RenderBuilder. | T2 | 4 |
| AB-VIS-009 | Photoreal rendering | Exports the current view as an Arc Scene to RenderBuilder and streams model changes to it; stills, panoramas and illuminance images come back over local communication. | T2 | 4 |
| AB-VIS-010 | Lighting | Uses light fixture families with photometric data files and light groups that can be dimmed together in renders. | T2 | 4 |
| AB-VIS-011 | Render representations | Gives entourage and planting families a render representation separate from their drawing representation. | T2 | 4 |
| AB-VIS-012 | Live visualization link | Streams the model to MeshBuilder for scene work, keeping element identity so updates replace previous geometry. | T3 | 4 |
