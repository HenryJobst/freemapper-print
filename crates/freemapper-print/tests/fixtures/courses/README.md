# Course fixtures

All files here are written by hand for freemapper-print on 2026-11-09 (M3 Task 2), following the public IOF
data standard 3.0 `CourseData` structure. They contain made-up data and are released under the same licence as
this repository.

The IOF datastandard-v3 examples were not copied: their licence could not be confirmed.

| File | Purpose |
|---|---|
| `y-up.xml` | Two courses; `MapPosition` values with y pointing up, as Purple Pen, Condes and OCAD write them. The reader keeps the values; detection happens later. Also `Position` (lat/lng), length and climb, a crossing point. |
| `text-position.xml` | A score course with a `MapTextPosition` (an offset from the circle centre) on a control. |
| `forked.xml` | Two courses of one `CourseFamily` (a fork), a plain course, and a course with alternative controls (a `CourseControl` with two `Control` children). |
