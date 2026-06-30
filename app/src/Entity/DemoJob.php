<?php

namespace App\Entity;

use App\Repository\DemoJobRepository;
use Doctrine\ORM\Mapping as ORM;

#[ORM\Entity(repositoryClass: DemoJobRepository::class)]
#[ORM\Table(name: 'demo_job')]
class DemoJob
{
    #[ORM\Id]
    #[ORM\Column(length: 36)]
    private string $id;

    #[ORM\Column(length: 64)]
    private string $username;

    #[ORM\Column(length: 16)]
    private string $status;

    // Filled by the message handler: a random greeting, translated in the
    // requester's locale. Null until the job is handled.
    #[ORM\Column(name: 'generated_sentence', length: 255, nullable: true)]
    private ?string $generatedSentence = null;

    #[ORM\Column(name: 'created_at')]
    private \DateTimeImmutable $createdAt;

    #[ORM\Column(name: 'completed_at', nullable: true)]
    private ?\DateTimeImmutable $completedAt = null;

    public function __construct(string $id, string $username)
    {
        $this->id = $id;
        $this->username = $username;
        $this->status = 'pending';
        $this->createdAt = new \DateTimeImmutable();
    }

    public function getId(): string
    {
        return $this->id;
    }

    public function getUsername(): string
    {
        return $this->username;
    }

    public function getStatus(): string
    {
        return $this->status;
    }

    public function getGeneratedSentence(): ?string
    {
        return $this->generatedSentence;
    }

    public function setGeneratedSentence(string $generatedSentence): void
    {
        $this->generatedSentence = $generatedSentence;
    }

    public function getCreatedAt(): \DateTimeImmutable
    {
        return $this->createdAt;
    }

    public function getCompletedAt(): ?\DateTimeImmutable
    {
        return $this->completedAt;
    }

    public function markCompleted(): void
    {
        $this->status = 'done';
        $this->completedAt = new \DateTimeImmutable();
    }
}
