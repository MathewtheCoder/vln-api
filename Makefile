.PHONY: default clean build_plugins pack

PLUGINS=vln
OUT_DIR=.build
NATIVE_PLUGINS=$(PLUGINS:%=${OUT_DIR}/plugins/%)
CODEDEPLOY_FILES=$(shell find -L .codedeploy -type f)
VALOR_BIN=~/.cargo/bin/valor_bin
VALOR_VER ?= 0.4.2-alpha.0

default: build_plugins

pack: app.zip

build_plugins: $(NATIVE_PLUGINS) 

clean: 
	rm -f $(NATIVE_PLUGINS) app.zip 

app.zip: $(OUT_DIR)/valor $(NATIVE_PLUGINS) 
	@zip app -j $(CODEDEPLOY_FILES)
	@zip app $<
	@zip app $(filter-out $<,$^)

target/release/lib%.so:
	cargo build -p $* --release

$(VALOR_BIN):
	cargo install valor_bin --version $(VALOR_VER) --target-dir target

$(OUT_DIR)/valor: $(VALOR_BIN)
	@mkdir -p $(@D); cp $< $@

$(OUT_DIR)/plugins/%: target/release/lib%.so
	@mkdir -p $(@D)
	mv $^ $@ 
