// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

use std::str::FromStr;
use std::time::Duration;

use futures::AsyncReadExt;
use futures::AsyncSeekExt;
use http::StatusCode;
use log::warn;
use reqwest::Url;
use sha2::Digest;
use sha2::Sha256;

use crate::*;

pub fn tests(op: &Operator, tests: &mut Vec<Trial>) {
    let cap = op.info().full_capability();

    if cap.read && cap.write {
        tests.extend(async_trials!(
            op,
            test_read_full,
            test_read_range,
            test_read_large_range,
            test_reader_range,
            test_reader_range_with_buffer,
            test_reader_from,
            test_reader_from_with_buffer,
            test_reader_tail,
            test_reader_tail_with_buffer,
            test_read_not_exist,
            test_read_with_if_match,
            test_read_with_if_none_match,
            test_read_with_dir_path,
            test_read_with_special_chars,
            test_read_with_override_cache_control,
            test_read_with_override_content_disposition,
            test_read_with_override_content_type,
            test_read_with_invalid_seek
        ))
    }

    if cap.read && !cap.write {
        tests.extend(async_trials!(
            op,
            test_read_only_read_full,
            test_read_only_read_full_with_special_chars,
            test_read_only_read_with_range,
            test_read_only_reader_with_range,
            test_read_only_reader_from,
            test_read_only_reader_tail,
            test_read_only_read_not_exist,
            test_read_only_read_with_dir_path,
            test_read_only_read_with_if_match,
            test_read_only_read_with_if_none_match
        ))
    }
}

/// Read full content should match.
pub async fn test_read_full(op: Operator) -> anyhow::Result<()> {
    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let bs = op.read(&path).await?;
    assert_eq!(size, bs.len(), "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    Ok(())
}

/// Read range content should match.
pub async fn test_read_range(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, length) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let bs = op.read_with(&path).range(offset..offset + length).await?;
    assert_eq!(bs.len() as u64, length, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!(
            "{:x}",
            Sha256::digest(&content[offset as usize..(offset + length) as usize])
        ),
        "read content"
    );

    Ok(())
}

/// Read large range content should match.
pub async fn test_read_large_range(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, _) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let bs = op.read_with(&path).range(offset..u32::MAX as u64).await?;
    assert_eq!(
        bs.len() as u64,
        size as u64 - offset,
        "read size with large range"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content[offset as usize..])),
        "read content with large range"
    );

    Ok(())
}

/// Read range content should match.
pub async fn test_reader_range(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, length) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = op.reader_with(&path).range(offset..offset + length).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!(
            "{:x}",
            Sha256::digest(&content[offset as usize..(offset + length) as usize])
        ),
        "read content"
    );

    Ok(())
}

/// Read range content should match.
pub async fn test_reader_range_with_buffer(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, length) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = op
        .reader_with(&path)
        .range(offset..offset + length)
        .buffer(4096)
        .await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!(
            "{:x}",
            Sha256::digest(&content[offset as usize..(offset + length) as usize])
        ),
        "read content"
    );

    Ok(())
}

/// Read range from should match.
pub async fn test_reader_from(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, _) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = op.reader_with(&path).range(offset..).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), size - offset as usize, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content[offset as usize..])),
        "read content"
    );

    Ok(())
}

/// Read range from should match.
pub async fn test_reader_from_with_buffer(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (offset, _) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = op.reader_with(&path).range(offset..).buffer(4096).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), size - offset as usize, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content[offset as usize..])),
        "read content"
    );

    Ok(())
}

/// Read range tail should match.
pub async fn test_reader_tail(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (_, length) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = match op.reader_with(&path).range(..length).await {
        Ok(r) => r,
        // Not all services support range with tail range, let's tolerate this.
        Err(err) if err.kind() == ErrorKind::Unsupported => {
            warn!("service doesn't support range with tail");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), length as usize, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content[size - length as usize..])),
        "read content"
    );

    Ok(())
}

/// Read range tail should match.
pub async fn test_reader_tail_with_buffer(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_range {
        return Ok(());
    }

    let (path, content, size) = TEST_FIXTURE.new_file(op.clone());
    let (_, length) = gen_offset_length(size);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = match op.reader_with(&path).range(..length).buffer(4096).await {
        Ok(r) => r,
        // Not all services support range with tail range, let's tolerate this.
        Err(err) if err.kind() == ErrorKind::Unsupported => {
            warn!("service doesn't support range with tail");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), length as usize, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content[size - length as usize..])),
        "read content"
    );

    Ok(())
}

/// Read not exist file should return NotFound
pub async fn test_read_not_exist(op: Operator) -> anyhow::Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let bs = op.read(&path).await;
    assert!(bs.is_err());
    assert_eq!(bs.unwrap_err().kind(), ErrorKind::NotFound);

    Ok(())
}

/// Read with if_match should match, else get a ConditionNotMatch error.
pub async fn test_read_with_if_match(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_if_match {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;

    let res = op.read_with(&path).if_match("\"invalid_etag\"").await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let bs = op
        .read_with(&path)
        .if_match(meta.etag().expect("etag must exist"))
        .await
        .expect("read must succeed");
    assert_eq!(bs, content);

    Ok(())
}

/// Read with if_none_match should match, else get a ConditionNotMatch error.
pub async fn test_read_with_if_none_match(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_if_none_match {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let meta = op.stat(&path).await?;

    let res = op
        .read_with(&path)
        .if_none_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let bs = op
        .read_with(&path)
        .if_none_match("\"invalid_etag\"")
        .await
        .expect("read must succeed");
    assert_eq!(bs, content);

    Ok(())
}

/// Read with dir path should return an error.
pub async fn test_read_with_dir_path(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().create_dir {
        return Ok(());
    }

    let path = TEST_FIXTURE.new_dir_path();

    op.create_dir(&path).await.expect("write must succeed");

    let result = op.read(&path).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::IsADirectory);

    Ok(())
}

/// Read file with special chars should succeed.
pub async fn test_read_with_special_chars(op: Operator) -> anyhow::Result<()> {
    // Ignore test for supabase until https://github.com/apache/opendal/issues/2194 addressed.
    if op.info().scheme() == opendal::Scheme::Supabase {
        warn!("ignore test for supabase until https://github.com/apache/opendal/issues/2194 is resolved");
        return Ok(());
    }
    // Ignore test for atomicserver until https://github.com/atomicdata-dev/atomic-server/issues/663 addressed.
    if op.info().scheme() == opendal::Scheme::Atomicserver {
        warn!("ignore test for atomicserver until https://github.com/atomicdata-dev/atomic-server/issues/663 is resolved");
        return Ok(());
    }

    let path = format!("{} !@#$%^&()_+-=;',.txt", uuid::Uuid::new_v4());
    let (path, content, size) = TEST_FIXTURE.new_file_with_path(op.clone(), &path);

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let bs = op.read(&path).await?;
    assert_eq!(size, bs.len(), "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        format!("{:x}", Sha256::digest(&content)),
        "read content"
    );

    Ok(())
}

/// Read file with override-cache-control should succeed.
pub async fn test_read_with_override_cache_control(op: Operator) -> anyhow::Result<()> {
    if !(op.info().full_capability().read_with_override_cache_control
        && op.info().full_capability().presign)
    {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_cache_control = "no-cache, no-store, must-revalidate";
    let signed_req = op
        .presign_read_with(&path, Duration::from_secs(60))
        .override_cache_control(target_cache_control)
        .await
        .expect("sign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get("cache-control")
            .expect("cache-control header must exist")
            .to_str()
            .expect("cache-control header must be string"),
        target_cache_control
    );

    Ok(())
}

/// Read file with override_content_disposition should succeed.
pub async fn test_read_with_override_content_disposition(op: Operator) -> anyhow::Result<()> {
    if !(op
        .info()
        .full_capability()
        .read_with_override_content_disposition
        && op.info().full_capability().presign)
    {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_content_disposition = "attachment; filename=foo.txt";

    let signed_req = op
        .presign_read_with(&path, Duration::from_secs(60))
        .override_content_disposition(target_content_disposition)
        .await
        .expect("presign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(http::header::CONTENT_DISPOSITION)
            .expect("content-disposition header must exist")
            .to_str()
            .expect("content-disposition header must be string"),
        target_content_disposition
    );
    assert_eq!(resp.bytes().await?, content);

    Ok(())
}

/// Read file with override_content_type should succeed.
pub async fn test_read_with_override_content_type(op: Operator) -> anyhow::Result<()> {
    if !(op.info().full_capability().read_with_override_content_type
        && op.info().full_capability().presign)
    {
        return Ok(());
    }

    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let target_content_type = "application/opendal";

    let signed_req = op
        .presign_read_with(&path, Duration::from_secs(60))
        .override_content_type(target_content_type)
        .await
        .expect("presign must succeed");

    let client = reqwest::Client::new();
    let mut req = client.request(
        signed_req.method().clone(),
        Url::from_str(&signed_req.uri().to_string()).expect("must be valid url"),
    );
    for (k, v) in signed_req.header() {
        req = req.header(k, v);
    }

    let resp = req.send().await.expect("send must succeed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(http::header::CONTENT_TYPE)
            .expect("content-type header must exist")
            .to_str()
            .expect("content-type header must be string"),
        target_content_type
    );
    assert_eq!(resp.bytes().await?, content);

    Ok(())
}

/// seeking a negative position should return a InvalidInput error
pub async fn test_read_with_invalid_seek(op: Operator) -> anyhow::Result<()> {
    let (path, content, _) = TEST_FIXTURE.new_file(op.clone());

    op.write(&path, content.clone())
        .await
        .expect("write must succeed");

    let mut r = op.reader(&path).await?;
    let res = r.seek(std::io::SeekFrom::Current(-1024)).await;

    assert!(res.is_err());

    assert_eq!(
        res.unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput,
        "seeking a negative position should return a InvalidInput error"
    );

    Ok(())
}

/// Read full content should match.
pub async fn test_read_only_read_full(op: Operator) -> anyhow::Result<()> {
    let bs = op.read("normal_file.txt").await?;
    assert_eq!(bs.len(), 30482, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5",
        "read content"
    );

    Ok(())
}

/// Read full content should match.
pub async fn test_read_only_read_full_with_special_chars(op: Operator) -> anyhow::Result<()> {
    let bs = op.read("special_file  !@#$%^&()_+-=;',.txt").await?;
    assert_eq!(bs.len(), 30482, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5",
        "read content"
    );

    Ok(())
}

/// Read full content should match.
pub async fn test_read_only_read_with_range(op: Operator) -> anyhow::Result<()> {
    let bs = op.read_with("normal_file.txt").range(1024..2048).await?;
    assert_eq!(bs.len(), 1024, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "330c6d57fdc1119d6021b37714ca5ad0ede12edd484f66be799a5cff59667034",
        "read content"
    );

    Ok(())
}

/// Read range should match.
pub async fn test_read_only_reader_with_range(op: Operator) -> anyhow::Result<()> {
    let mut r = op.reader_with("normal_file.txt").range(1024..2048).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), 1024, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "330c6d57fdc1119d6021b37714ca5ad0ede12edd484f66be799a5cff59667034",
        "read content"
    );

    Ok(())
}

/// Read from should match.
pub async fn test_read_only_reader_from(op: Operator) -> anyhow::Result<()> {
    let mut r = op.reader_with("normal_file.txt").range(29458..).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), 1024, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "cc9312c869238ea9410b6716e0fc3f48056f2bfb2fe06ccf5f96f2c3bf39e71b",
        "read content"
    );

    Ok(())
}

/// Read tail should match.
pub async fn test_read_only_reader_tail(op: Operator) -> anyhow::Result<()> {
    let mut r = op.reader_with("normal_file.txt").range(..1024).await?;

    let mut bs = Vec::new();
    r.read_to_end(&mut bs).await?;

    assert_eq!(bs.len(), 1024, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "cc9312c869238ea9410b6716e0fc3f48056f2bfb2fe06ccf5f96f2c3bf39e71b",
        "read content"
    );

    Ok(())
}

/// Read not exist file should return NotFound
pub async fn test_read_only_read_not_exist(op: Operator) -> anyhow::Result<()> {
    let path = uuid::Uuid::new_v4().to_string();

    let bs = op.read(&path).await;
    assert!(bs.is_err());
    assert_eq!(bs.unwrap_err().kind(), ErrorKind::NotFound);

    Ok(())
}

/// Read with dir path should return an error.
pub async fn test_read_only_read_with_dir_path(op: Operator) -> anyhow::Result<()> {
    let path = format!("{}/", uuid::Uuid::new_v4());

    let result = op.read(&path).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), ErrorKind::IsADirectory);

    Ok(())
}

/// Read with if_match should match, else get a ConditionNotMatch error.
pub async fn test_read_only_read_with_if_match(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_if_match {
        return Ok(());
    }

    let path = "normal_file.txt";

    let meta = op.stat(path).await?;

    let res = op.read_with(path).if_match("invalid_etag").await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let bs = op
        .read_with(path)
        .if_match(meta.etag().expect("etag must exist"))
        .await
        .expect("read must succeed");
    assert_eq!(bs.len(), 30482, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5",
        "read content"
    );

    Ok(())
}

/// Read with if_none_match should match, else get a ConditionNotMatch error.
pub async fn test_read_only_read_with_if_none_match(op: Operator) -> anyhow::Result<()> {
    if !op.info().full_capability().read_with_if_none_match {
        return Ok(());
    }

    let path = "normal_file.txt";

    let meta = op.stat(path).await?;

    let res = op
        .read_with(path)
        .if_none_match(meta.etag().expect("etag must exist"))
        .await;
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::ConditionNotMatch);

    let bs = op
        .read_with(path)
        .if_none_match("invalid_etag")
        .await
        .expect("read must succeed");
    assert_eq!(bs.len(), 30482, "read size");
    assert_eq!(
        format!("{:x}", Sha256::digest(&bs)),
        "943048ba817cdcd786db07d1f42d5500da7d10541c2f9353352cd2d3f66617e5",
        "read content"
    );

    Ok(())
}
