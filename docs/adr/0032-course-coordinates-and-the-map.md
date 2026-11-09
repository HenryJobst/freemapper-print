# 0032. Course coordinates and the map

Date: 2026-11-09
Status: Accepted

## Context

A course file gives each control a map position in mm from the map's origin, and IOF XML often adds latitude
and longitude. Writers disagree on the y axis. Purple Pen, Condes and OCAD write y up, while the IOF standard and
OOM count y down. Nothing in the file says which one was used. A course printed with the wrong axis or on the
wrong map puts runners at the wrong place, so a doubtful fit has to stop the print (spec §5.3).

## Decision

- A map scale stated in the course file (IOF XML `Map/Scale`, the `.ppen` map scale) must equal the map's scale
  denominator, or the fit stops with `ScaleMismatch`. Positions are never rescaled. A file without a scale
  gets the note `ScaleUnchecked`. A map that states no scale (denominator 0) against a file that states one is
  the error `MapScaleUnknown`.
- The project file can set the file's y axis (`[sources] course_y_axis`, `"up"` or `"down"`;
  `FitOptions.y_axis`). The set axis must still fit, otherwise the error is `OutsideMap`.
- Purple Pen always writes y up (ruling R36). Without a project value a `.ppen` is read y up and nothing else
  (`SourceFormat::y_axis`): a file that does not fit y up is `OutsideMap`, never read mirrored, and no note asks
  about the axis. A project value against the format still decides, with the warning `AxisAgainstFormat`, so a
  wrong `course_y_axis = "down"` cannot print a mirrored course unnoticed.
- For IOF XML without a project value, course fitting tries both readings of a file point, `(x, −y)` and
  `(x, y)`, times 1000 to get OOM's µm. A reading fits when every control lies inside the box around all object
  coordinates of the map, grown by 10 mm on every side. The box uses the object coordinates, not the rendered
  extent, so it costs one pass over the map. The 10 mm let a start or finish sit just outside the drawn map, as
  courses on a cut-out often do.
- One fitting reading is used. When both fit and nothing decides between them, y up wins, because most
  course-setting programs write it, and the warning `OrientationAssumed` asks the user to confirm the axis in the
  project file. That holds also when every control has only latitude and longitude but the file has bends,
  number offsets, circle gaps or special objects, which still follow the axis. `YUpDetected` is kept for a y up
  that only one reading or latitude and longitude showed. There is no `Ambiguous` error: this rule always picks
  one.
- No fitting reading is the error `OutsideMap` with the number of controls on the map.
- The map counts as georeferenced when it has a CRS text and a projected reference point. Only UTM is
  supported: `EPSG:258NN` (ETRS89), `EPSG:326NN` and `EPSG:327NN` (WGS84), the same as `+init=epsg:…`, and
  `+proj=utm +zone=NN` on WGS84 or GRS80, optionally `+south`. Any other CRS skips the check with the note
  `GeorefSkipped`. The projection is transverse Mercator with Krüger's series to n⁴ (EPSG Guidance Note 7-2).
  ETRS89 and WGS84 are treated as the same datum; they differ by under a metre in Europe.
- Map and grid coordinates convert the way OOM does. OOM's `Georeferencing::updateTransformation`
  (`src/core/georeferencing.cpp`) builds `translate(ref) · rotate(−grivation) · scale(s, −s) ·
  translate(−map_ref)`, and Qt's `QTransform::rotate(a)` maps (x, y) to (x·cos a − y·sin a, x·sin a + y·cos a).
  So map mm less the map reference point are multiplied by scale denominator and combined scale factor, y is
  flipped, the result is turned clockwise by the grivation, and the projected reference point is added. Map
  up then points along the grid bearing equal to the grivation, which is magnetic north. The map reader keeps
  the combined scale factor (`grid_scale_factor`) and the map reference point (`<ref_point>` directly under
  `<georeferencing>`, (0, 0) when missing).
- With a georeferenced map and latitude and longitude on controls that have a map position, latitude and
  longitude check every reading that fits. Readings beyond the tolerance drop out. If two remain, for
  instance with all such controls near the map's x axis, the axis counts as undecided (the rule above). A
  control more than 20 m from where its latitude and longitude put it is the error `GeorefMismatch`, naming the
  control.
  Latitude and longitude in a course file often come from a GNSS reading in the field, good to 5 or 10 m, and
  ETRS89 and WGS84 differ by under a metre. A wrong map, zone or datum shifts every control by hundreds of
  metres. 20 m sits between the two. Shifting the course to make it fit would hide such an error, so the fit
  refuses.
- The project file can switch latitude and longitude off (`[sources] course_georef_check = false`;
  `FitOptions.check_georef`). They then neither check nor decide anything, the note is `GeorefCheckOff`, and
  controls with only latitude and longitude are the error `NoPositionNoGeoref`.
- A control with latitude and longitude and no map position is placed through the map's georeferencing. If the
  map cannot do that, the error `NoPositionNoGeoref` names the controls and the reason.
- The fit turns circle gap angles with the axis too: when it flips y, `(a, b)` becomes `(−b, −a)`. After the
  fit the event is entirely in the map frame (`Stop::circle_gaps`).
- Number text offsets take only the linear part of the reading, the y flip. Positions from latitude and
  longitude never act on offsets: an offset stays in the file's mm, which share the map's orientation.
- `fit` returns the fitted event together with its notes (`Fit { event, notes }`), because `CourseEvent` has no
  place for notes about the map.

## Consequences

- A course on the wrong map or with the wrong axis does not print. On a georeferenced UTM map with latitude and
  longitude in the file, the axis is never guessed.
- A Purple Pen project needs no `course_y_axis`; `init --courses` writes none.
- Without latitude and longitude, an IOF XML course near the map's middle fits both readings and y up is taken.
  A file written y down with such a course prints upside down unless the user heeds `OrientationAssumed` and
  sets `course_y_axis`.
- After `GeorefMismatch` there are two ways on. If the map's georeferencing is wrong, fix it in OOM. If the
  course file's latitude and longitude are wrong or not wanted, set `course_georef_check = false` in the project
  file; the map positions alone then place the course.
- Maps in Gauss-Krüger, a national grid or a local CRS get no latitude and longitude check yet.

Source: M3 plan Task 3, the controller rulings R6, R7, R9, R10, R11 and R36, the Task 3 review and the final
review of M3 (I1).
