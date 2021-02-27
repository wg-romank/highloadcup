REMOTE_TAG=invisible_frog
REMOTE_REPO=stor.highloadcup.ru/rally
IMAGE_NAME=hlcup-rust

test:
	cd hlcup && cargo test

run:
	docker run -it ${IMAGE_NAME}

build:
	cd hlcup && cargo build --release

submit: test
	cd hlcup/ && cargo build --release
	docker build -t ${IMAGE_NAME} --build-arg TS="$(shell date)" .
	docker tag ${IMAGE_NAME} ${REMOTE_REPO}/${REMOTE_TAG}
	docker push ${REMOTE_REPO}/${REMOTE_TAG}
