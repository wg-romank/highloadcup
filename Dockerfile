FROM rust

COPY hlcup/target/release/hlcup /app/
WORKDIR /app/
RUN chmod +x ./hlcup
ENTRYPOINT ["./hlcup"]
