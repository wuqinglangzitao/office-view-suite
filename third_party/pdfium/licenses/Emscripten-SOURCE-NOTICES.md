# Emscripten 6.0.0 supplemental source notices

Source commit: afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3

Verbatim copyright/license comment excerpts from generic musl source, libc++ Ryu, Emscripten JavaScript runtime sources, and the dlmalloc public-domain declaration. Repeated identical comments are grouped. This is a conservative notice set for the toolchain; inclusion does not assert that every listed function survived linking into this WASM. The corresponding project license texts are supplied separately.

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/__cexp.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/__cexpf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/cexp.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/cexpf.c#L2

```text
/*-
 * Copyright (c) 2011 David Schultz <das@FreeBSD.ORG>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/catan.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/catanf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/catanl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__polevll.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/erfl.c#L12
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/expl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/expm1l.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/lgammal.c#L12
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log10l.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log1pl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2l.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/logl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/powl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/tgammal.c#L2

```text
/*
 * Copyright (c) 2008 Stephen L. Moshier <steve@moshier.net>
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/ccosh.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/ccoshf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/csinh.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/csinhf.c#L2

```text
/*-
 * Copyright (c) 2005 Bruce D. Evans and Steven G. Kargl
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice unmodified, this list of conditions, and the following
 *    disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/csqrt.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/csqrtf.c#L2

```text
/*-
 * Copyright (c) 2007 David Schultz <das@FreeBSD.ORG>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/ctanh.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/complex/ctanhf.c#L2

```text
/*-
 * Copyright (c) 2011 David Schultz
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice unmodified, this list of conditions, and the following
 *    disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR ``AS IS'' AND ANY EXPRESS OR
 * IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED.
 * IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT
 * NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
 * DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
 * THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
 * (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF
 * THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_blowfish.c#L5

```text
/*
 * The crypt_blowfish homepage is:
 *
 *	http://www.openwall.com/crypt/
 *
 * This code comes from John the Ripper password cracker, with reentrant
 * and crypt(3) interfaces added, but optimizations specific to password
 * cracking removed.
 *
 * Written by Solar Designer <solar at openwall.com> in 1998-2012.
 * No copyright is claimed, and the software is hereby placed in the public
 * domain.  In case this attempt to disclaim copyright and place the software
 * in the public domain is deemed null and void, then the software is
 * Copyright (c) 1998-2014 Solar Designer and it is hereby released to the
 * general public under the following terms:
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted.
 *
 * There's ABSOLUTELY NO WARRANTY, express or implied.
 *
 * It is my intent that you should be able to use this on your system,
 * as part of a software package, or anywhere else to improve security,
 * ensure compatibility, or for any other purpose.  I would appreciate
 * it if you give credit where it is due and keep your modifications in
 * the public domain as well, but I don't require that in order to let
 * you place this code and any modifications you make under a license
 * of your choice.
 *
 * This implementation is fully compatible with OpenBSD's bcrypt.c for prefix
 * "$2b$", originally by Niels Provos <provos at citi.umich.edu>, and it uses
 * some of his ideas.  The password hashing algorithm was designed by David
 * Mazieres <dm at lcs.mit.edu>.  For information on the level of
 * compatibility for bcrypt hash prefixes other than "$2b$", please refer to
 * the comments in BF_set_key() below and to the included crypt(3) man page.
 *
 * There's a paper on the algorithm that explains its design decisions:
 *
 *	http://www.usenix.org/events/usenix99/provos.html
 *
 * Some of the tricks in BF_ROUND might be inspired by Eric Young's
 * Blowfish library (I can't be sure if I would think of something if I
 * hadn't seen his code).
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_des.c#L18

```text
/*
 * FreeSec: libcrypt for NetBSD
 *
 * Copyright (c) 1994 David Burren
 * Copyright (c) 2000,2002,2010,2012 Solar Designer
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 * 3. Neither the name of the author nor the names of other contributors
 *    may be used to endorse or promote products derived from this software
 *    without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 *	$Owl: Owl/packages/glibc/crypt_freesec.c,v 1.6 2010/02/20 14:45:06 solar Exp $
 *	$Id: crypt.c,v 1.15 1994/09/13 04:58:49 davidb Exp $
 *
 * This is an original implementation of the DES and the crypt(3) interfaces
 * by David Burren.  It has been heavily re-worked by Solar Designer.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_md5.c#L12

```text
/* public domain md5 implementation based on rfc1321 and libtomcrypt */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_md5.c#L155

```text
/*-
 * Copyright (c) 2003 Poul-Henning Kamp
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_sha256.c#L1

```text
/*
 * public domain sha256 crypt implementation
 *
 * original sha crypt design: http://people.redhat.com/drepper/SHA-crypt.txt
 * in this implementation at least 32bit int is assumed,
 * key length is limited, the $5$ prefix is mandatory, '\n' and ':' is rejected
 * in the salt and rounds= setting must contain a valid iteration count,
 * on error "*" is returned.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_sha256.c#L16

```text
/* public domain sha256 implementation based on fips180-3 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_sha512.c#L1

```text
/*
 * public domain sha512 crypt implementation
 *
 * original sha crypt design: http://people.redhat.com/drepper/SHA-crypt.txt
 * in this implementation at least 32bit int is assumed,
 * key length is limited, the $6$ prefix is mandatory, '\n' and ':' is rejected
 * in the salt and rounds= setting must contain a valid iteration count,
 * on error "*" is returned.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/crypt/crypt_sha512.c#L16

```text
/* public domain sha512 implementation based on fips180-3 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/internal/progname.c#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/internal/proxying_notification_state.h#L1

```text
/*
 * Copyright 2022 The Emscripten Authors.  All rights reserved.
 * Emscripten is available under two separate licenses, the MIT license and the
 * University of Illinois/NCSA Open Source License.  Both these licenses can be
 * found in the LICENSE file.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__cos.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__rem_pio2_large.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__sin.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/acos.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/acosl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/asin.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/asinl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/j0.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/j1.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/jn.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log10.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2_small.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log_small.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/scalb.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__cosdf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__rem_pio2f.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__sindf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/acosf.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/asinf.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atan.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atan2f.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atanf.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atanl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/cbrtf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/cos.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/cosf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/erf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/erff.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/erfl.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/expm1.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/expm1f.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/j0f.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/j1f.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/jnf.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/lgammaf_r.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/lgammal.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log10f.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log1p.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log1pf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/scalbf.c#L5
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/sin.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/sincos.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/sincosf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/sinf.c#L6
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/tan.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/tanf.c#L6

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__cosl.c#L3
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__sinl.c#L3

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 * Copyright (c) 2008 Steven G. Kargl, David Schultz, Bruce D. Evans.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__rem_pio2.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 * Optimized by Bruce D. Evans.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__rem_pio2l.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 * Copyright (c) 2008 Steven G. Kargl, David Schultz, Bruce D. Evans.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 * Optimized by Bruce D. Evans.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__tan.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__tandf.c#L6

```text
/*
 * ====================================================
 * Copyright 2004 Sun Microsystems, Inc.  All Rights Reserved.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/__tanl.c#L3

```text
/*
 * ====================================================
 * Copyright 2004 Sun Microsystems, Inc.  All Rights Reserved.
 * Copyright (c) 2008 Steven G. Kargl, David Schultz, Bruce D. Evans.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atan2.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/atan2l.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/lgamma_r.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunSoft, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/cbrt.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 * Optimized by Bruce D. Evans.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/cbrtl.c#L2

```text
/*-
 * ====================================================
 * Copyright (C) 1993 by Sun Microsystems, Inc. All rights reserved.
 * Copyright (c) 2009-2011, Bruce D. Evans, Steven G. Kargl, David Schultz.
 *
 * Developed at SunPro, a Sun Microsystems, Inc. business.
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 *
 * The argument reduction and testing for exceptional cases was
 * written by Steven G. Kargl with input from Bruce D. Evans
 * and David A. Schultz.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp.c#L1

```text
/*
 * Double-precision e^x function.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp2.c#L1

```text
/*
 * Double-precision 2^x function.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp2f.c#L1

```text
/*
 * Single-precision 2^x function.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp2f_data.c#L1

```text
/*
 * Shared data between expf, exp2f and powf.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp2f_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2f_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/logf_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/powf.c#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/powf_data.h#L1

```text
/*
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp2l.c#L2

```text
/*-
 * Copyright (c) 2005-2008 David Schultz <das@FreeBSD.ORG>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp_data.c#L1

```text
/*
 * Shared data between exp, exp2 and pow.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/exp_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log_data.h#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/pow_data.h#L1

```text
/*
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/expf.c#L1

```text
/*
 * Single-precision e^x function.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/fmaf.c#L2
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/fmal.c#L2

```text
/*-
 * Copyright (c) 2005-2011 David Schultz <das@FreeBSD.ORG>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE AUTHOR AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log.c#L1

```text
/*
 * Double-precision log(x) function.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2.c#L1

```text
/*
 * Double-precision log2(x) function.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2_data.c#L1

```text
/*
 * Data for log2.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2f.c#L1

```text
/*
 * Single-precision log2 function.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log2f_data.c#L1

```text
/*
 * Data definition for log2f.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/log_data.c#L1

```text
/*
 * Data for log.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/logf.c#L1

```text
/*
 * Single-precision log function.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/logf_data.c#L1

```text
/*
 * Data definition for logf.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/pow.c#L1

```text
/*
 * Double-precision x^y function.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/pow_data.c#L1

```text
/*
 * Data for the log part of pow.
 *
 * Copyright (c) 2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/pow_small.c#L2

```text
/*
 * ====================================================
 * Copyright (C) 2004 by Sun Microsystems, Inc. All rights reserved.
 *
 * Permission to use, copy, modify, and distribute this
 * software is freely granted, provided that this notice
 * is preserved.
 * ====================================================
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/math/powf_data.c#L1

```text
/*
 * Data definition for powf.
 *
 * Copyright (c) 2017-2018, Arm Limited.
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/misc/fmtmsg.c#L1

```text
/* Public domain fmtmsg()
 * Written by Isaac Dunham, 2014
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/regex/regcomp.c#L1

```text
/*
  regcomp.c - TRE POSIX compatible regex compilation functions.

  Copyright (c) 2001-2009 Ville Laurikari <vl@iki.fi>
  All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions
  are met:

    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in the
       documentation and/or other materials provided with the distribution.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER AND CONTRIBUTORS
  ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
  LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
  A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT
  HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
  LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
  THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*/
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/regex/regexec.c#L1

```text
/*
  regexec.c - TRE POSIX compatible matching functions (and more).

  Copyright (c) 2001-2009 Ville Laurikari <vl@iki.fi>
  All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions
  are met:

    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in the
       documentation and/or other materials provided with the distribution.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER AND CONTRIBUTORS
  ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
  LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
  A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT
  HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
  LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
  THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*/
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/regex/tre-mem.c#L1

```text
/*
  tre-mem.c - TRE memory allocator

  Copyright (c) 2001-2009 Ville Laurikari <vl@iki.fi>
  All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions
  are met:

    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in the
       documentation and/or other materials provided with the distribution.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER AND CONTRIBUTORS
  ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
  LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
  A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT
  HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
  LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
  THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*/
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/regex/tre.h#L1

```text
/*
  tre-internal.h - TRE internal definitions

  Copyright (c) 2001-2009 Ville Laurikari <vl@iki.fi>
  All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions
  are met:

    1. Redistributions of source code must retain the above copyright
       notice, this list of conditions and the following disclaimer.

    2. Redistributions in binary form must reproduce the above copyright
       notice, this list of conditions and the following disclaimer in the
       documentation and/or other materials provided with the distribution.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDER AND CONTRIBUTORS
  ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
  LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
  A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT
  HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
  LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
  DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
  THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
  OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

*/
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libc/musl/src/stdlib/qsort.c#L1

```text
/* Copyright (C) 2011 by Lynn Ochs
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * IN THE SOFTWARE.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2fixed.cpp#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2s.cpp#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/f2s.cpp#L1

```text
//===----------------------------------------------------------------------===//
//
// Part of the LLVM Project, under the Apache License v2.0 with LLVM Exceptions.
// See https://llvm.org/LICENSE.txt for license information.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
//
//===----------------------------------------------------------------------===//
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2fixed.cpp#L9
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2s.cpp#L9
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/f2s.cpp#L9

```text
// Copyright (c) Microsoft Corporation.
// SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2fixed.cpp#L12
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2s.cpp#L12
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/f2s.cpp#L12

```text
// Copyright 2018 Ulf Adams
// Copyright (c) Microsoft Corporation. All rights reserved.
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2fixed.cpp#L24
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2s.cpp#L24
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/f2s.cpp#L24

```text
// The copyright notices in the Software and this entire statement, including
// the above license grant, this restriction and the following disclaimer,
// must be included in all copies of the Software, in whole or in part, and
// all derivative works of the Software, unless such copies or derivative
// works are solely in the form of machine-executable object code generated by
// a source language processor.
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2fixed.cpp#L31
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/d2s.cpp#L31
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/libcxx/src/ryu/f2s.cpp#L31

```text
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE, TITLE AND NON-INFRINGEMENT. IN NO EVENT
// SHALL THE COPYRIGHT HOLDERS OR ANYONE DISTRIBUTING THE SOFTWARE BE LIABLE
// FOR ANY DAMAGES OR OTHER LIABILITY, WHETHER IN CONTRACT, TORT OR OTHERWISE,
// ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libaddfunction.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libbase64.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libexports.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libint53.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libstrings.js#L1

```text
/**
 * @license
 * Copyright 2020 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libasync.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libglew.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libhtml5.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libhtml5_webgl.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libtrace.js#L1

```text
/**
 * @license
 * Copyright 2014 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libatomic.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libfetchfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libicasefs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libjsfilefs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libopfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libpromise.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasm_worker.js#L1

```text
/**
 * @license
 * Copyright 2023 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libautodebug.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libccall.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libgetvalue.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libnodepath.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasmfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasmfs_fetch.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasmfs_jsimpl.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasmfs_node.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasmfs_opfs.js#L1

```text
/**
 * @license
 * Copyright 2022 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libbootstrap.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libidbstore.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/liblz4.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libpthread_stub.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libsyscall.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libworkerfs.js#L1

```text
/**
 * @license
 * Copyright 2015 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libbrowser.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwget.js#L1

```text
/**
 * @license
 * Copyright 2011 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libcore.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libeventloop.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libexceptions.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libglemu.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libsdl.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwebgl.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwebgl2.js#L1

```text
/**
 * @license
 * Copyright 2010 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libdylink.js#L1

```text
/**
 * @license
 * Copyright 2020 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 *
 * Dynamic library loading
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libegl.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libglut.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libxlib.js#L1

```text
/**
 * @license
 * Copyright 2012 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libembind.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libemval.js#L1

```text
// Copyright 2012 The Emscripten Authors.  All rights reserved.
// Emscripten is available under two separate licenses, the MIT license and the
// University of Illinois/NCSA Open Source License.  Both these licenses can be
// found in the LICENSE file.
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libembind_gen.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libembind_shared.js#L1

```text
// Copyright 2023 The Emscripten Authors.  All rights reserved.
// Emscripten is available under two separate licenses, the MIT license and the
// University of Illinois/NCSA Open Source License.  Both these licenses can be
// found in the LICENSE file.
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libexceptions_stub.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libstack_trace.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwasi.js#L1

```text
/**
 * @license
 * Copyright 2019 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libfetch.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libproxyfs.js#L1

```text
/**
 * @license
 * Copyright 2016 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libglfw.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libidbfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libmemfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libnodefs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libopenal.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libpath.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libsockfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libtty.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libuuid.js#L1

```text
/**
 * @license
 * Copyright 2013 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libfs_shared.js#L1

```text
/**
 * @license
 * Copyright 2032 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/liblegacy.js#L1

```text
/**
 * @license
 * Copyright 2010 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 *
 * Legacy library symbols that are no longer used by emscripten itself but
 * could have external users.
 *
 * Symbols in this file are not available in `-sSTRICT` mode.
 *
 * Any usage of symbols in this file will result in a `-Wdeprecated` warning.
 *
 * Symbol in this file should be removed after "enough time" has passed such
 * that all external users have been able to transition away.
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libnoderawfs.js#L1
- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libwebsocket.js#L1

```text
/**
 * @license
 * Copyright 2018 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libpipefs.js#L1

```text
/**
 * @license
 * Copyright 2017 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libpthread.js#L1

```text
/**
 * @license
 * Copyright 2015 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 *
 * Because only modern JS engines support SAB we can use modern JS language
 * features within this file (ES2020).
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/src/lib/libtime.js#L1

```text
/**
 * @license
 * Copyright 2024 The Emscripten Authors
 * SPDX-License-Identifier: MIT
 */
```

## Sources

- https://github.com/emscripten-core/emscripten/blob/afa15e0c56d1292e073c2c91bafc1d5e0cdf0dd3/system/lib/dlmalloc.c#L54

```text
/*
 This is a version (aka dlmalloc) of malloc/free/realloc written by
 Doug Lea and released to the public domain, as explained at
 http://creativecommons.org/publicdomain/zero/1.0/ Send questions,
 comments, complaints, performance data, etc to dl@cs.oswego.edu
 
 * Version 2.8.6 Wed Aug 29 06:57:58 2012  Doug Lea
 Note: There may be an updated version of this malloc obtainable at
 ftp://gee.cs.oswego.edu/pub/misc/malloc.c
 Check before installing!
```
