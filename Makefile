.PHONY: default clean build_plugins pack

PLUGINS=blockchain
OUT_DIR=out
NATIVE_PLUGINS=$(PLUGINS:%=${OUT_DIR}/%.so)
CODEDEPLOY_FILES=$(shell find -L .codedeploy -type f)
VALOR_BIN=$(shell which valor)

default: build_plugins

pack: app.zip

build_plugins: $(NATIVE_PLUGINS) 

clean: 
	rm -f $(NATIVE_PLUGINS) app.zip 

app.zip: $(NATIVE_PLUGINS) 
	@zip app -j $(CODEDEPLOY_FILES)
	@zip app -j $(VALOR_BIN)
	@zip app $^

target/release/lib%.so:
	cargo build -p $* --release

$(OUT_DIR)/%.so: target/release/lib%.so
	@mkdir -p ${OUT_DIR}
	mv $^ $@ 
