#!/usr/bin/env python3
"""
gen_wrapped_headers.py — Python replacement for wrapawk / wrapawk_macosx.

Reads wrapfunc.inp and writes four header files into the output directory:
  wrapped.h    — extern declarations of next_* / my_* function pointers
  wrapdef.h    — definitions (assignments) of those pointers
  wrapstruct.h — next_wrap[] / INTERPOSE table
  wraptmpf.h   — temporary stub functions (load_library_symbols + forward)

Usage:
  gen_wrapped_headers.py <wrapfunc.inp> <outdir> <mode>
  mode: "linux" or "macosx"
"""

import sys
import os

WARNING = "/* Automatically generated file. Do not edit. Edit wrapfunc.inp. */"


def open_out(outdir, name):
    return open(os.path.join(outdir, name), "w")


def process_linux(lines, outdir):
    hdr = open_out(outdir, "wrapped.h")
    dfn = open_out(outdir, "wrapdef.h")
    strt = open_out(outdir, "wrapstruct.h")
    tmp = open_out(outdir, "wraptmpf.h")

    for f in (hdr, dfn, strt, tmp):
        f.write(WARNING + "\n")

    hdr.write("#ifndef WRAPPED_H\n#define WRAPPED_H\n")
    dfn.write("#ifndef WRAPDEF_H\n#define WRAPDEF_H\n")
    tmp.write("#ifndef WRAPTMPF_H\n#define WRAPTMPF_H\n")
    strt.write("#ifndef WRAPSTRUCT_H\n#define WRAPSTRUCT_H\n")
    strt.write("struct next_wrap_st next_wrap[]= {\n")

    for line in lines:
        line = line.rstrip("\n")

        # Skip C-style comment lines (lines that are purely /* ... */ comments).
        # NOTE: preprocessor directives like '#endif /* FOO */' must NOT be skipped;
        # they are handled by the '#' check below.
        stripped = line.strip()
        if stripped.startswith('/*'):
            continue

        # Preprocessor directives — copy verbatim to all files
        if line.startswith("#"):
            for f in (strt, tmp, dfn, hdr):
                f.write(line + "\n")
            continue

        # Blank lines — copy verbatim
        if not line.strip():
            for f in (strt, hdr, dfn, tmp):
                f.write(line + "\n")
            continue

        # Data lines: wrapawk only accepts records with at least 4
        # semicolon-separated fields (name;ret;argtype;argname;...)
        fields = line.split(";")
        if len(fields) < 4:
            continue

        name = fields[0].strip()
        ret = fields[1].strip()
        argtype = fields[2].strip()
        argname = fields[3].strip()
        macro = fields[4].strip() if len(fields) > 4 else ""
        openat_extra = fields[5].strip() if len(fields) > 5 else ""

        # Skip empty names and commented-out entries (e.g. /*opendir;DIR *;...;...*/)
        if not name or name.startswith('/'):
            continue
        if not argname:
            continue

        if openat_extra:
            # openat-style: variadic via va_list
            strt.write('  {{(void(*))&next_{n}, "{n}"}},\n'.format(n=name))
            hdr.write("extern {r} (*next_{n}){o};\n".format(r=ret, n=name, o=openat_extra))
            dfn.write("{r} (*next_{n}){o}=tmp_{n};\n".format(r=ret, n=name, o=openat_extra))
            tmp.write("{r} tmp_{n}{o}{{\n".format(r=ret, n=name, o=openat_extra))
            tmp.write("  mode_t mode = 0;\n")
            tmp.write("  if (flags & O_CREAT) {\n")
            tmp.write("    va_list args;\n")
            tmp.write("    va_start(args, flags);\n")
            tmp.write("    mode = va_arg(args, int);\n")
            tmp.write("    va_end(args);\n")
            tmp.write("  }\n")
            tmp.write("  load_library_symbols();\n")
            tmp.write("  return  next_{n} {a};\n".format(n=name, a=argname))
            tmp.write("}\n\n")

        elif macro:
            strt.write(
                '  {{(void(*))&NEXT_{M}_NOARG, {n}_QUOTE}},\n'.format(M=macro, n=name)
            )
            hdr.write(
                "extern {r} (*NEXT_{M}_NOARG){a};\n".format(r=ret, M=macro, a=argtype)
            )
            dfn.write(
                "{r} (*NEXT_{M}_NOARG){a}=TMP_{M};\n".format(r=ret, M=macro, a=argtype)
            )
            tmp.write("{r} TMP_{M} {a}{{\n".format(r=ret, M=macro, a=argtype))
            tmp.write("  load_library_symbols();\n")
            tmp.write(
                "  return  NEXT_{M}_NOARG {an};\n".format(M=macro, an=argname)
            )
            tmp.write("}\n\n")

        else:
            strt.write('  {{(void(*))&next_{n}, "{n}"}},\n'.format(n=name))
            hdr.write("extern {r} (*next_{n}){a};\n".format(r=ret, n=name, a=argtype))
            dfn.write("{r} (*next_{n}){a}=tmp_{n};\n".format(r=ret, n=name, a=argtype))
            tmp.write("{r} tmp_{n} {a}{{\n".format(r=ret, n=name, a=argtype))
            tmp.write("  load_library_symbols();\n")
            tmp.write("  return  next_{n} {an};\n".format(n=name, an=argname))
            tmp.write("}\n\n")

    strt.write("  {NULL, NULL},\n};\n#endif\n")
    tmp.write("#endif\n")
    dfn.write("#endif\n")
    hdr.write("#endif\n")

    for f in (hdr, dfn, strt, tmp):
        f.close()


def process_macosx(lines, outdir):
    hdr = open_out(outdir, "wrapped.h")
    dfn = open_out(outdir, "wrapdef.h")
    strt = open_out(outdir, "wrapstruct.h")
    tmp = open_out(outdir, "wraptmpf.h")

    for f in (hdr, dfn, strt, tmp):
        f.write(WARNING + "\n")

    hdr.write("#ifndef WRAPPED_H\n#define WRAPPED_H\n")
    hdr.write("#define MY_GLUE2(a,b) a ## b\n")
    hdr.write("#define MY_DEF(a) MY_GLUE2(my_,a)\n")
    dfn.write("#ifndef WRAPDEF_H\n#define WRAPDEF_H\n")
    tmp.write("#ifndef WRAPTMPF_H\n#define WRAPTMPF_H\n")
    strt.write("#ifndef WRAPSTRUCT_H\n#define WRAPSTRUCT_H\n")
    strt.write("typedef struct interpose_s {\n")
    strt.write("  void *new_func;\n")
    strt.write("  void *orig_func;\n")
    strt.write("} interpose_t;\n")
    strt.write("#define INTERPOSE(newf,oldf) \\\n")
    strt.write(
        '  __attribute__((used)) static const interpose_t MY_GLUE2(_interpose_,oldf) \\\n'
    )
    strt.write(
        '    __attribute__((section("__DATA,__interpose"))) = {(void *)newf, (void *)oldf}\n'
    )
    strt.write("\n")

    for line in lines:
        line = line.rstrip("\n")

        stripped = line.strip()
        if stripped.startswith('/*'):
            continue

        if line.startswith("#"):
            for f in (strt, tmp, dfn, hdr):
                f.write(line + "\n")
            continue

        if not line.strip():
            for f in (strt, hdr, dfn, tmp):
                f.write(line + "\n")
            continue

        fields = line.split(";")
        if len(fields) < 3:
            continue

        name = fields[0].strip()
        ret = fields[1].strip()
        argtype = fields[2].strip()
        argname = fields[3].strip() if len(fields) > 3 else ""
        macro = fields[4].strip() if len(fields) > 4 else ""
        argtype_def = fields[5].strip() if len(fields) > 5 else ""

        # Skip empty names and commented-out entries (e.g. /*opendir;...;...*/)
        if not name or name.startswith('/'):
            continue

        if not argtype_def:
            argtype_def = argtype

        if macro:
            hdr.write(
                'extern {r} MY_DEF({n}){a} __attribute__((visibility("hidden")));\n'.format(
                    r=ret, n=name, a=argtype
                )
            )
            strt.write(
                "INTERPOSE(MY_DEF({n}_RAW),{n}_RAW);\n".format(n=name)
            )
            dfn.write("#undef {n}\n".format(n=name))
            dfn.write("#define {n} MY_DEF({n}_RAW)\n".format(n=name))
            tmp.write("extern {r} {n} {a};\n".format(r=ret, n=name, a=argtype_def))
            tmp.write(
                "static __inline__ {r} NEXT_{M}_NOARG {a} __attribute__((always_inline));\n".format(
                    r=ret, M=macro, a=argtype
                )
            )
            tmp.write(
                "static __inline__ {r} NEXT_{M}_NOARG {a} {{\n".format(
                    r=ret, M=macro, a=argtype
                )
            )
            tmp.write("  return {n} {an};\n".format(n=name, an=argname))
            tmp.write("}\n\n")

        else:
            hdr.write(
                'extern {r} my_{n} {a} __attribute__((visibility("hidden")));\n'.format(
                    r=ret, n=name, a=argtype_def
                )
            )
            strt.write("#undef {n}\n".format(n=name))
            strt.write("INTERPOSE(my_{n},{n});\n".format(n=name))
            strt.write("#define {n} my_{n}\n".format(n=name))
            dfn.write("#define {n} my_{n}\n".format(n=name))
            tmp.write("extern {r} {n} {a};\n".format(r=ret, n=name, a=argtype_def))
            if argname:
                tmp.write(
                    "static __inline__ {r} next_{n} {a} __attribute__((always_inline));\n".format(
                        r=ret, n=name, a=argtype
                    )
                )
                tmp.write(
                    "static __inline__ {r} next_{n} {a} {{\n".format(
                        r=ret, n=name, a=argtype
                    )
                )
                tmp.write("  return {n} {an};\n".format(n=name, an=argname))
                tmp.write("}\n")
            tmp.write("\n")

    strt.write("\nstruct next_wrap_st next_wrap[]= {\n")
    strt.write("  {NULL, NULL},\n};\n#endif\n")
    tmp.write("#endif\n")
    dfn.write("#endif\n")
    hdr.write("#endif\n")

    for f in (hdr, dfn, strt, tmp):
        f.close()


def main():
    if len(sys.argv) != 4:
        print(
            "Usage: gen_wrapped_headers.py <wrapfunc.inp> <outdir> <linux|macosx>",
            file=sys.stderr,
        )
        sys.exit(1)

    inp_path, outdir, mode = sys.argv[1], sys.argv[2], sys.argv[3]

    with open(inp_path) as f:
        lines = f.readlines()

    os.makedirs(outdir, exist_ok=True)

    if mode == "macosx":
        process_macosx(lines, outdir)
    else:
        process_linux(lines, outdir)


if __name__ == "__main__":
    main()
