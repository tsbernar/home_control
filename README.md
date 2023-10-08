Use cross to compile for the pi if you don't want to wait for long compilation times on the pi itself.

CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target arm-unknown-linux-gnueabihf
