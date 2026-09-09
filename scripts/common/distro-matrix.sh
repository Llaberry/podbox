#!/usr/bin/env bash
# The distribution matrix: the pinned rows, the qualification rule, and the
# tally. Sourced by every experiment that asks one question of many rootfs.
#
# ⭐ TODO/milestones.md T-1203 owns the mechanics and T-1106 owns the M5 row
# list. This file exists because T-1106 says to drive M5 through T-1203's runner
# rather than writing a second one: two runners drift, and the one that drifts
# is the one nobody is looking at.
#
# ⛔ WHAT IS SHARED IS THE MECHANICS, NOT THE ENGINE. `125-across-distributions.sh`
# drives each row with `docker run`, because its subject is what a DISTRIBUTION
# does and docker is the instrument. `240-distro-sweep.sh` drives each row with
# `podbox run`, because its subject is podbox. The pinning, the `no-pull` row,
# the exit-2-when-nothing-ran rule and the reference qualification are the same
# in both and live here.
#
# Four rules, and each is a line below:
#
#   1. the manifest DIGEST is the pin. A rolling tag measures a different thing
#      each week and says so nowhere;
#   2. ⚠ a row that could not be pulled reads `no-pull`, is counted apart from a
#      row whose command ran and failed, and does not on its own fail the run.
#      "Unreachable" and "broken" need different next moves;
#   3. ⛔ a run where NOTHING ran exits 2. Every row unreachable reads exactly
#      like every row agreeing, and the second is the hoped-for answer;
#   4. the reference is fully qualified before it is pulled. An unqualified name
#      resolves through an engine's own shortname aliases to a different
#      registry, where a digest does not exist, and that arrives as `manifest
#      unknown`, which reads as a broken pin.

# ---------------------------------------------------------------- the rows
#
# REFERENCE|LOCAL-NAME|LIBC|MANIFEST-DIGEST
#
# ⛔ Two lists, and they are two because they answer two questions against two
# sets of evidence. Both are here so that "which images does this project
# measure against" has one answer.

# T-1203's set, for the `nsswitch` question (TODO/complete.md T-0410).
# ⛔ Docker Hub, and the readings in `experiments/results/across/` were taken
# against exactly these digests on 2026-09-08. They are not migrated to another
# registry: the same tag at another registry is another image, and moving them
# would silently replace a committed measurement rather than repeat it.
# ⚠ Docker Hub's rate limit is attributed to a shared address in this
# environment, so a run of 125- can legitimately read `no-pull` on every row.
# That is rule 3's case and it exits 2.
DISTRO_ROWS_NSSWITCH='
alpine:3.22|alpine-3.22|musl|sha256:7c8cb692ae09657cbc4a3f3cbd0e8d5a2690ba38386aaaf252dbb060bf5eb2e6
alpine:3.20|alpine-3.20|musl|sha256:c64c687cbea9300178b30c95835354e34c4e4febc4badfe27102879de0483b5e
alpine:3.10|alpine-3.10|musl|sha256:e515aad2ed234a5072c4d2ef86a1cb77d5bfe4b11aa865d9214875734c4eeb3c
voidlinux/voidlinux-musl:latest|voidlinux-musl|musl|sha256:d5c970d0015c3aa2559a2b5a87b158839969fbb1941a9ef52cc484a5392554cd
debian:11|debian-11|glibc|sha256:c0a2ad73611131275b0e9a2e7544cfe3725f4268d4085d8c6f61fdd55aef7917
debian:12|debian-12|glibc|sha256:2f65600e1252c5649d2213e1d1ea4d74253d26514dc6530102a875e429245929
ubuntu:20.04|ubuntu-20.04|glibc|sha256:c664f8f86ed5a386b0a340d981b8f81714e21a8b9c73f658c4bea56aa179d54a
rockylinux:8|rockylinux-8|glibc|sha256:2d05a9266523bbf24f33ebc3a9832e4d5fd74b973c220f2204ca802286aa275d
opensuse/leap:15.6|opensuse-leap-15.6|glibc|sha256:ca2942f9510c3e30fd322017782cdf6c067b2183dd7d56b39d0c697a9808ce2a
fedora:42|fedora-42|glibc|sha256:7c63468daf71fdc5bda3699cd483b169bb995b5137265d5ffe8f04e2ce87fbb8
archlinux:latest|archlinux-latest|glibc|sha256:818793c894d94534c22f2149154a39ebaee57e4e67321023b0866a1d5722036c
'

# ⭐ M5's set: TOOL.md section 5 M5's ten distributions, each chosen because one
# of TODO/complete.md's fixups was missing when it broke.
#
# ⛔ NO DOCKER HUB. Every reference is `ghcr.io`, `public.ecr.aws` or the
# distribution's own registry, because Docker Hub's rate limit is attributed to
# a shared address in the environments this runs in and a limited pull reads as
# a broken registry rather than as a quota. ⚠ `public.ecr.aws/almalinux/almalinux`
# does not exist; almalinux is taken from its own quay.io organisation, which is
# where the project publishes it.
#
# ⛔ Every digest below was resolved with `docker buildx imagetools inspect` on
# 2026-09-09. Re-pulling a tag without editing its row silently changes what
# every number describes.
DISTRO_ROWS_M5='
public.ecr.aws/docker/library/alpine:3.20|alpine|musl|sha256:d9e853e87e55526f6b2917df91a2115c36dd7c696a35be12163d44e6e2a4b6bc
public.ecr.aws/debian/debian:bookworm-slim|debian|glibc|sha256:833d7afe7d42e2fc552740ebdb947218770eb6f0a533927ed2a04b4d453e4f0a
public.ecr.aws/ubuntu/ubuntu:24.04|ubuntu|glibc|sha256:41938337738ac39ec015eecf7ac5ed9add869fffeb394f2b90819ba6793d889b
ghcr.io/pkgforge-dev/archlinux:latest|archlinux|glibc|sha256:b2507f1964270cab3cc190aa8df858521f6a7e45e969146a012877891d5dfc9b
quay.io/almalinuxorg/almalinux:9|almalinux|glibc|sha256:3da647417303590fea18439c87421a886d4245fef358cf131d0e0ce7b23f4236
quay.io/rockylinux/rockylinux:9|rocky|glibc|sha256:8101994123cf3d0a8fee517bee7f39e555c7d92bd2d9eb3303cc988a0eeed00f
quay.io/rockylinux/rockylinux:9-minimal|rocky-minimal|glibc|sha256:e1d0a9f5ed99d52e7faf03afe7ee32e48b231c4dd9586808b3d1aedf894dff04
quay.io/fedora/fedora:42|fedora|glibc|sha256:e78cd1a688cd079c23864f289a89a49a3f4ad66d817864e325e1d058310ee95c
registry.opensuse.org/opensuse/leap:15.6|opensuse-leap|glibc|sha256:e4d84bbc5e0fa0df64381e95ad7ea8e3867cc34f3b1d174e9948d0f559d223e2
ghcr.io/void-linux/void-musl:latest|voidlinux-musl|musl|sha256:af9308c79fd14ddc7b53e181caee312d182c89186792dc1ef5b8215865b444ac
'

# distro_qualify REFERENCE -> a fully qualified name with no tag
#
# ⚠ Rule 4. A bare `alpine` resolves through an engine's shortname aliases; a
# name with a dot or a colon in its first component is already a registry.
distro_qualify() {
	case "$1" in
	*.*/* | *:*/*) printf '%s' "${1%:*}" ;;
	*/*) printf 'docker.io/%s' "${1%:*}" ;;
	*) printf 'docker.io/library/%s' "${1%%:*}" ;;
	esac
}

# distro_pinned REFERENCE DIGEST -> the reference to actually pull
distro_pinned() {
	printf '%s@%s' "$(distro_qualify "$1")" "$2"
}

# distro_count_rows ROWS -> how many rows the list carries
distro_count_rows() {
	printf '%s' "$1" | grep -c .
}

# distro_verdict RAN NOPULL BROKEN
#
# ⛔ Rules 2 and 3, in one place. Prints the verdict block and RETURNS the exit
# code the caller should use; it never exits itself, so a caller can still write
# its result file.
distro_verdict() {
	local ran="$1" nopull="$2" broken="$3"
	echo
	echo "== rows"
	printf '  ran %s, no-pull %s, harness-failed %s\n' "$ran" "$nopull" "$broken"
	echo
	echo "== verdict"
	if [ "$ran" -eq 0 ]; then
		echo "  NOTHING RAN. Every row unreachable reads exactly like every row"
		echo "  agreeing, so this exits 2 rather than 0."
		return 2
	fi
	if [ "$broken" -gt 0 ]; then
		echo "  $broken row(s) pulled and the subject could not run there."
		return 1
	fi
	echo "  $ran row(s) produced a reading. The table above is the measurement;"
	echo "  a difference between rows is a difference between distributions."
	return 0
}
