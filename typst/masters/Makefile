.PHONY: all watch clean preview-titlepage preview-disclaimer

MAIN = main.typ
OUT = out/thesis.pdf

all: $(OUT)

$(OUT): $(MAIN) thesis.typ content lib
	@mkdir -p out
	typst compile $(MAIN) $(OUT)

watch:
	@mkdir -p out
	typst watch $(MAIN) $(OUT)

preview-titlepage:
	@mkdir -p out
	typst compile dev/preview-titlepage.typ out/preview-titlepage.pdf

preview-disclaimer:
	@mkdir -p out
	typst compile dev/preview-disclaimer.typ out/preview-disclaimer.pdf

clean:
	rm -rf out .typst-cache
