CREATE TABLE plugin_versions (
 plugin_id VARCHAR(191) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 version VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 sha256 BINARY(32) NOT NULL,
 size_bytes BIGINT UNSIGNED NOT NULL,
 object_key VARCHAR(1024) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 status ENUM('active','withdrawn','revoked') NOT NULL,
 published_at DATETIME(6) NOT NULL,
 PRIMARY KEY (plugin_id,version),
 KEY idx_plugin_versions_published (plugin_id,published_at)
) ENGINE=InnoDB;
CREATE TABLE plugin_stats (
 plugin_id VARCHAR(191) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 attempts BIGINT UNSIGNED NOT NULL DEFAULT 0,
 downloads BIGINT UNSIGNED NOT NULL DEFAULT 0,
 updated_at DATETIME(6) NOT NULL,
 PRIMARY KEY (plugin_id)
) ENGINE=InnoDB;
CREATE TABLE plugin_version_stats (
 plugin_id VARCHAR(191) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 version VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 attempts BIGINT UNSIGNED NOT NULL DEFAULT 0,
 downloads BIGINT UNSIGNED NOT NULL DEFAULT 0,
 updated_at DATETIME(6) NOT NULL,
 PRIMARY KEY (plugin_id,version),
 CONSTRAINT fk_version_stats FOREIGN KEY (plugin_id,version) REFERENCES plugin_versions(plugin_id,version)
) ENGINE=InnoDB;
CREATE TABLE download_attempts (
 attempt_id BINARY(16) NOT NULL,
 plugin_id VARCHAR(191) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 version VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 sha256 BINARY(32) NOT NULL,
 size_bytes BIGINT UNSIGNED NOT NULL,
 receipt_kid VARCHAR(32) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 issued_at DATETIME(6) NOT NULL,
 expires_at DATETIME(6) NOT NULL,
 confirmed_at DATETIME(6) NULL,
 status ENUM('issued','confirmed') NOT NULL DEFAULT 'issued',
 PRIMARY KEY (attempt_id),
 KEY idx_attempts_expiry (expires_at),
 KEY idx_attempts_confirmed (confirmed_at),
 CONSTRAINT fk_attempt_version FOREIGN KEY (plugin_id,version) REFERENCES plugin_versions(plugin_id,version)
) ENGINE=InnoDB;
CREATE TABLE catalog_state (
 id TINYINT UNSIGNED NOT NULL,
 digest BINARY(32) NOT NULL,
 source_commit VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL,
 generation BIGINT UNSIGNED NOT NULL,
 imported_at DATETIME(6) NOT NULL,
 counted_since DATETIME(6) NOT NULL,
 PRIMARY KEY (id),
 CONSTRAINT chk_catalog_singleton CHECK (id=1)
) ENGINE=InnoDB;
INSERT INTO catalog_state (id,digest,source_commit,generation,imported_at,counted_since)
VALUES (1,UNHEX(REPEAT('00',32)),'',0,UTC_TIMESTAMP(6),UTC_TIMESTAMP(6));
