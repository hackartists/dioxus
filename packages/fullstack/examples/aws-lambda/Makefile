build-Function:

build-sam:
	@sam build
	cargo run -p dioxus-cli -- build --release --platform fullstack --features lambda
	cp -rf dist/* .aws-sam/build/Function/
	mv .aws-sam/build/Function/dagit .aws-sam/build/Function/bootstrap
