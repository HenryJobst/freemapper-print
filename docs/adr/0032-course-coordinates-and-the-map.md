# 0032. Course coordinates and the map

Date: 2026-11-09
Status: Accepted

## Context

A course file gives each control a map position in mm from the map's origin, and IOF XML often adds latitude
and longitude. Writers disagree on the y axis. Purple Pen, Condes and OCAD write y up, while the IOF standard and
OOM count y down. Nothing in the file says which one was used. A course printed with the wrong axis or on the
wrong map puts runners at the wrong place, so a doubtful fit has to stop the print (spec §5.3).

## Decision

- Course fitting tries both readings of a file point, `(x, −y)` and `(x, y)`, times 1000 to get OOM's µm.
  A reading fits when every control lies inside the box around all object coordinates of the map, grown by
  10 mm on every side. The box uses the object coordinates, not the rendered extent, so it costs one pass over
  the map. The 10 mm let a start or finish sit just outside the drawn map, as courses on a cut-out often do.
- One fitting reading is used. With both fitting and no latitude and longitude to decide, y up wins, because
  most course-setting programs write it, and the note `YUpDetected` says so. There is no `Ambiguous` error:
  this rule always picks one.
- No fitting reading is the error `OutsideMap` with the number of controls on the map.
- The map counts as georeferenced when it has a CRS text and a projected reference point. Only UTM is
  supported: `EPSG:258NN` (ETRS89), `EPSG:326NN` and `EPSG:327NN` (WGS84), the same as `+init=epsg:…`, and
  `+proj=utm +zone=NN` on WGS84 or GRS80, optionally `+south`. Any other CRS skips the check with the note
  `GeorefSkipped`. The projection is transverse Mercator with Krüger's series to n⁴ (EPSG Guidance Note 7-2).
  ETRS89 and WGS84 are treated as the same datum; they differ by under a metre in Europe.
- Map and grid coordinates convert the way OOM does (`Georeferencing::updateTransformation`): map mm less the
  map reference point, times scale denominator and combined scale factor, y flipped, turned by the grivation,
  plus the projected reference point. The reader now keeps the combined scale factor (`grid_scale_factor`) and
  the map reference point (`<ref_point>` directly under `<georeferencing>`, (0, 0) when missing).
- With a georeferenced map and latitude and longitude on controls that have a map position, latitude and
  longitude decide between the fitting readings (y up first) and check the chosen one. A control more than
  20 m from where its latitude and longitude put it is the error `GeorefMismatch`, naming the control.
  Latitude and longitude in a course file often come from a GNSS reading in the field, good to 5 or 10 m, and
  ETRS89 and WGS84 differ by under a metre. A wrong map, zone or datum shifts every control by hundreds of
  metres. 20 m sits between the two. Shifting the course to make it fit would hide such an error, so the fit
  refuses.
- A control with latitude and longitude and no map position is placed through the map's georeferencing. If the
  map cannot do that, the error `NoPositionNoGeoref` names the controls and the reason.
- Number text offsets take only the linear part of the reading, the y flip. Positions from latitude and
  longitude never act on offsets: an offset stays in the file's mm, which share the map's orientation.
- `fit` returns the fitted event together with its notes (`Fit { event, notes }`), because `CourseEvent` has no
  place for notes about the map.
- A file whose `Map/Scale` differs from the map's scale is not rescaled. The box and the latitude and longitude
  check catch most such files; a later task may add a scale check.

## Consequences

- A course on the wrong map or with the wrong axis does not print. On a georeferenced UTM map with latitude and
  longitude in the file, the axis is never guessed.
- Without latitude and longitude, a course near the map's middle fits both readings and y up is taken. A file
  written y down with such a course prints upside down, and the note `YUpDetected` is the only hint.
- Maps in Gauss-Krüger, a national grid or a local CRS get no latitude and longitude check yet.

Source: M3 plan Task 3 and the controller rulings R6 and R7.
