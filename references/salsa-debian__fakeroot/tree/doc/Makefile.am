AUTOMAKE_OPTIONS=foreign
if HAVE_PO4A
SUBDIRS = de es fr nl pt ro sv
endif

man_MANS = faked.1 fakeroot.1

#another automake bug workaround
EXTRA_DIST = $(man_MANS) README.saving

PO4A_STAMP = po4a-translated.stamp
PO4A_CFG = $(srcdir)/po4a/po4a.cfg
PO4A_INPUTS = \
	$(PO4A_CFG) \
	$(srcdir)/po4a/po/*.po \
	$(srcdir)/po4a/addendum.1.* \
	$(srcdir)/faked.1 \
	$(srcdir)/fakeroot.1

if HAVE_PO4A
BUILT_SOURCES = $(PO4A_STAMP)
CLEANFILES = $(PO4A_STAMP)

$(PO4A_STAMP): $(PO4A_INPUTS)
	$(AM_V_GEN)$(PO4A) -k 0 --no-update --variable "srcdir=$(realpath $(srcdir))/" $(PO4A_CFG)
	$(AM_V_at)touch $@
endif
