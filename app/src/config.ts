// Runtime config from Vite env (app/.env.local). Program ids: agents/CONTRACTS.md, section 1.
import { address } from "@solana/kit";

export type DataSource = "mock" | "chain";

export const DATA_SOURCE: DataSource = import.meta.env.VITE_DATA === "chain" ? "chain" : "mock";
export const RPC_URL: string = import.meta.env.VITE_RPC_URL ?? "http://127.0.0.1:8899";
export const RPC_WS_URL: string | undefined = import.meta.env.VITE_RPC_WS_URL;
/** Wallet Standard chain id. */
export const CHAIN = (import.meta.env.VITE_CHAIN ?? "solana:devnet") as `${string}:${string}`;
/** Sprite book ids to load, comma separated (FORMAT.md 3.3). */
export const SPRITE_BOOK_IDS: number[] = (import.meta.env.VITE_SPRITE_BOOKS ?? "1")
  .split(",")
  .map((s: string) => Number(s.trim()))
  .filter((n: number) => Number.isInteger(n) && n >= 0 && n <= 0xffff);

export const PROGRAMS = {
  registry: address("dZ1jraMfS3bAUTkRw2TA1ddg8UDGzgUdFQz5pmBxjoG"),
  render: address("DgQCsXAe7Q7h8zdtToiTvk8XqNjjbXuD6xith2BmEiRB"),
  character: address("9uVNvkt5kTMad65GtdqQmBb3X1JQMWwQ985qLwTCdqfL"),
  items: address("48FtbrDegNrAUHFza9HdJzZAvUo3uUNqs1kNnZxFsqJS"),
  map: address("2YeyBfQ5xkRKD2kagxRS2yZYCQdWt5qidikaKfQhLBXH"),
  session: address("3gzfih2WaBgXcYZ57jWokmZM3FDE6QHX5y4R9EvKZoNJ"),
  city: address("2Cr5vB5awqDVx6XRcxtFFYQEG6Zy3aSAcKcqzh1xbVjS"),
} as const;

export const MPL_CORE_PROGRAM = address("CoREENxT6tW1HoK8ypY1SxRMZTcVPm7R94rH4PZNhX7d");
export const SYSTEM_PROGRAM = address("11111111111111111111111111111111");
