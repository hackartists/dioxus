build-Function:

.aws-sam:
	sam build
	cargo run -p dioxus-cli -- build --release --platform fullstack
	cp -rf dist .aws-sam/build/Function/
	mv .aws-sam/build/Function/dist/aws-lambda .aws-sam/build/Function/bootstrap

run: clean .aws-sam
	sam local start-api

clean:
	rm -rf dist .aws-sam

hard-clean: clean
	cargo clean
