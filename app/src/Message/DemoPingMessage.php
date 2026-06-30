<?php

namespace App\Message;

final readonly class DemoPingMessage
{
    public const TOPIC = 'app://demo';

    /**
     * Translation keys the handler picks from at random. They all carry a
     * `%username%` placeholder and live in translations/messages.<locale>.yaml.
     */
    public const GREETINGS = [
        'greeting.welcome',
        'greeting.try_async',
        'greeting.explore',
        'greeting.persisted',
        'greeting.native',
    ];

    public function __construct(
        public string $jobId,
        public string $username,
        // The worker runs outside any HTTP request, so it has no request locale.
        // We carry the requester's locale in the message and pass it explicitly to
        // the translator — the canonical way to do i18n in an async handler.
        public string $locale,
    ) {
    }
}
