<?php

namespace App\Message;

final readonly class DemoPingMessage
{
    public const TOPIC = 'app://demo';

    public function __construct(
        public string $jobId,
        public int $createdAt,
    ) {
    }
}
