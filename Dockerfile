FROM rust

ARG TS

WORKDIR /app/
RUN date > dummy_${TS}
ADD hlcup/target/release/hlcup /app/
RUN chmod +x ./hlcup
ENTRYPOINT ["./hlcup"]
