import { integer, sqliteTable, text } from "drizzle-orm/sqlite-core";
export const waitlist = sqliteTable("waitlist", {
  id: text("id").primaryKey(),
  email: text("email").notNull().unique(),
  platform: text("platform"),
  removalTokenHash: text("removal_token_hash").notNull().unique(),
  createdAt: integer("created_at").notNull(),
});
export const signupLimits = sqliteTable("signup_limits", {
  key: text("key").primaryKey(),
  attempts: integer("attempts").notNull().default(1),
  expiresAt: integer("expires_at").notNull(),
});
