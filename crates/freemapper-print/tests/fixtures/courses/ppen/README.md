# Purple Pen fixtures

A test event from Purple Pen's repository, copied unchanged on 2026-11-09 (M3 Task 4). The control
description tests use it. Two more files from the same commit, `lincoln-park.ppen` and `overlapper.ppen`,
moved to freemapper-core with the reader tests (ADR 0043).

Source: https://github.com/petergolde/PurplePen, commit `201417f97bb69302e3e0db123f428870e0914515`
(2026-11-07).

| File here | Path in the Purple Pen repository | SHA-256 | What it covers |
|---|---|---|---|
| `sampleevent8.ppen` | `src/TestFiles/descriptions/sampleevent8.ppen` | `d8d4872f…f1fb` | Description standard 2018, full description rows including column F text (`5x5\|3`), a map-exchange control, map exchanges at normal controls (`map-exchange="true"`), a water special and a descriptions special. |

Full hash: `d8d4872f040f56dc9ec2c8728e70e530d63331ee5cb19cb399e2abbec288f1fb`.

The files carry no licence header of their own. Purple Pen as a whole is published under the BSD licence below:
every source file in the repository carries it (for example `src/PurplePenCore/EventDB.cs`, "Copyright (c)
2006-2008, Peter Golde"), and the user documentation states it as the Purple Pen licence
(`doc/userdocs/WebSite/license.htm`, "Copyright © 2006-2007, Peter Golde"). The repository has no top-level
licence file. The tests read the file as it is.

## Licence

```
Copyright (c) 2006-2008, Peter Golde
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are
met:

1. Redistributions of source code must retain the above copyright
notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
notice, this list of conditions and the following disclaimer in the
documentation and/or other materials provided with the distribution.

3. Neither the name of Peter Golde, nor "Purple Pen", nor the names
of its contributors may be used to endorse or promote products
derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
OF SUCH DAMAGE.
```
